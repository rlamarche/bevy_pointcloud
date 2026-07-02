# Material Management: Extraction & Pipeline Specialization

To render point clouds with custom visual behaviors, `bevy_point_cloud` mirrors Bevy's type-erased material architecture. This page documents how materials are registered, how data is extracted from the main simulation world, and how their render pipelines are specialized, highlighting our custom implementation of a **dual mesh vertex layout**.


## 1. The Entry Point: `MaterialPlugin<M>`

The cornerstone of the material management system is the generic **`MaterialPlugin<M>`**. It is responsible for injecting the necessary assets, reflection types, and scheduling the systems required to synchronize material state between the Main World and the Render World.

```rust
pub struct MaterialPlugin<M: Material> {
    /// Debugging flags that can optionally be set when constructing the renderer.
    pub debug_flags: RenderDebugFlags,
    pub _marker: PhantomData<M>,
}
```

### Main App Initialization (`PostUpdate`)
Inside the main Bevy application, the plugin registers the material asset type, tracks specialized component states, and hooks up dirty-checking logic:
* Registers `M` as a standard Bevy `Asset` via `.init_asset::<M>()`.
* Registers the **`PointCloudMaterial3d<M>`** component for reflection.
* Initializes the main-world **`EntitiesNeedingSpecialization<M>`** tracking resource.
* Automatically registers system dependencies to mark material assets and their associated point cloud meshes as changed upon modification (`check_entities_needing_specialization`).

### Render App Orchestration (`ExtractSchedule` & `RenderStartup`)
If a sub-render app is found, the plugin hooks into the rendering loop lifecycle. It inserts bind group allocators at `RenderStartup` and registers the heavy data extraction routines to execute inside the `ExtractSchedule`.


## 2. Material Extraction & Systems Scheduling

During the **`ExtractSchedule`**, the plugin moves material data from the Main World into the Render World. Handled explicitly by the `MaterialPlugin<M>`, the following systems run sequentially to manage this synchronization:

### Extraction Systems & Order

#### `extract_mesh_materials::<M>`
* **Scheduling:** Runs inside Bevy's standard `MaterialExtractionSystems`.
* **Responsibility:** Mapping visible point cloud entities to their corresponding material asset IDs within the render world.
* **Core Logic:** Iterates over point cloud entities where either the `ViewVisibility` or the `PointCloudMaterial3d<M>` component has changed. 
  * If the entity is **visible**, it inserts its mapping into the **`RenderPointCloudMaterialInstances`** cache, tracking the untyped `AssetId` of the material along with the current change tick.
  * If the entity is **not visible**, it proactively removes it from the cache.
* **Why it matters:** This tracks the live link between individual scene entities and their chosen materials, allowing the renderer to know exactly which material to apply to each point cloud instance during the drawing phase.

#### `early_sweep_material_instances::<M>`
* **Scheduling:** Runs explicitly `.after(MaterialExtractionSystems).before(late_sweep_material_instances)`.
* **Responsibility:** Safely purging removed material associations from the render cache without disrupting same-frame material swaps.
* **Core Logic:** Iterates over all entities that had their `PointCloudMaterial3d<M>` component removed this frame. It checks the **`RenderPointCloudMaterialInstances`** cache and deletes the entry **only if it hasn't been updated during the current frame** (by comparing the entry's `last_change_tick` with the cache's current tick).
* **Why it matters:** In Bevy, swapping Material A for Material B on the same entity in a single frame triggers a removal event for Material A. If this system blindly deleted the entity's entry, it would accidentally destroy the newly extracted Material B data. Using change ticks ensures the system only sweeps truly obsolete materials while preserving hot-swapped ones.

#### `late_sweep_material_instances`
* **Scheduling:** Runs explicitly after all invocations of `early_sweep_material_instances` have completed.
* **Responsibility:** Performing a final cleanup of material instances for deleted or toggled point clouds, and incrementing the material cache change tick for the next frame.
* **Core Logic:** Iterates over all entities that had their core `PointCloud3d` component removed this frame. 
  * Similar to the early sweep, it checks the **`RenderPointCloudMaterialInstances`** cache and removes the entry **only if it wasn't updated during the current frame** (by verifying `last_change_tick`).
  * Once the loop finishes, it **bumps the global `current_change_tick`** by 1.
* **Why it matters:** This system serves two critical purposes. First, it handles the edge case where a point cloud's visibility or component structure was removed and re-added in the exact same frame, ensuring the active material data isn't accidentally destroyed. Second, because it is non-generic (unlike the per-material early sweep), it acts as the centralized choke point to safely increment the change tick exactly once per frame, readying the cache for the next cycle.

#### `extract_entities_needs_specialization::<M>`
* **Scheduling:** Runs inside Bevy's `DirtySpecializationSystems::CheckForChanges`.
* **Responsibility:** Transferring the list of main-world entities that require material or pipeline specialization into the render world's tracking system.
* **Core Logic:** Extracts the **`EntitiesNeedingSpecialization<M>`** resource from the main app and iterates through its `changed` list. For each main-world entity found, it registers its render-world counterpart (`MainEntity`) into the **`DirtySpecializations::changed_renderables`** map.
* **Why it matters:** When a point cloud entity's material state or layout becomes "dirty" in the main world, Bevy's rendering pipeline needs to evaluate if a new, specialized GPU pipeline (shader variations, bind groups, vertex attributes) needs to be compiled or assigned. This system bridges that state, ensuring the `RenderApp` knows exactly which renderables must undergo pipeline re-evaluation.

#### `extract_entities_that_need_specializations_removed::<M>`
* **Scheduling:** Runs inside Bevy's `DirtySpecializationSystems::CheckForRemovals`.
* **Responsibility:** Transferring the list of main-world entities whose material or pipeline specializations are no longer valid into the render world's tracking system.
* **Core Logic:** Extracts the **`EntitiesNeedingSpecialization<M>`** resource from the main app and iterates through its `removed` list. For each main-world entity found, it registers its render-world counterpart (`MainEntity`) into the **`DirtySpecializations::removed_renderables`** map.
* **Why it matters:** When a point cloud entity is destroyed, or its material is stripped away, any specialized GPU pipeline variations associated with it become dead weight. This system ensures that the `RenderApp` is notified of these removals, allowing the pipeline cache to flag them and cleanly purge or skip stale specialization configurations.


## 3. The `MaterialPipelineKey<M>`

Pipeline compilation is heavy. To avoid compiling a new GPU pipeline every frame, Bevy and `bevy_point_cloud` use a caching mechanism driven by a unique hashing key: the **`MaterialPipelineKey<M>`**.

```rust
pub struct MaterialPipelineKey<M: Material> {
    pub mesh_key: MeshPipelineKey,
    pub bind_group_data: M::Data,
}
```

Whenever a point cloud chunk needs to be drawn, a `MaterialPipelineKey` is constructed:
* **`mesh_key`**: A bitmask (`MeshPipelineKey`) capturing the global rendering state (e.g., MSAA sample count, HDR settings, view projection types, and whether the mesh layout has specific vertex attributes).
* **`bind_group_data`**: A lightweight, hashable data structure (`M::Data`) defined by your custom material (via the `AsBindGroup` trait). It stores configuration flags that affect shading paths (e.g., `HAS_TEXTURE`, `USE_GRADIENT`) but doesn't store the raw uniform values themselves.

If two chunks share the exact same `mesh_key` and `bind_group_data`, they produce the same `MaterialPipelineKey` hash, allowing the plugin to instantly reuse an already compiled GPU `RenderPipeline`.


## 4. Pipeline Specialization with Dual Layouts

The core algorithmic deviation from standard Bevy mesh rendering happens during pipeline specialization inside the **`RenderSystems::Specialize`** phase.

### `init_material_pipeline`
* **Scheduling:** Runs in `RenderStartup`, explicitly `.after(PointCloudPipelineSystems)`.
* **Responsibility:** Sets up the base `MaterialPipeline` resource, which wraps the global `PointCloudPipeline`.

### `specialize_material_meshes`
* **Scheduling:** Runs during the `Render` phase, inside `RenderSystems::Specialize`. It is heavily constrained to ensure data readiness:
  ```rust
  specialize_material_meshes
      .in_set(RenderSystems::Specialize)
      .after(prepare_assets::<RenderMesh>)
      .after(collect_meshes_for_gpu_building)
      .after(set_mesh_motion_vector_flags)
  ```
* **The Dual-Layout Resolution:** While standard Bevy resolves a pipeline using a single vertex layout, this system fetches **two distinct vertex layouts** and passes them to `Material::specialize`:
  1. **The Shape Layout (`shape_layout`):** Represents the vertex structure of a *single point primitive* (e.g., vertex positions forming a quad billboard or a cylinder shape). Resolved via `Material::shape_mesh()`.
  2. **The Instance Layout (`instance_layout`):** Represents the vertex structure of the *point cloud chunk data* (the heavy arrays of point positions, colors, LIDAR intensities or other custom attributes).
* **The Output:** These two layouts are mapped into **two distinct vertex buffer slots** (Slot 0 for the shape geometry at a per-vertex step rate, Slot 1 for the point data at a per-instance step rate) within the final `RenderPipelineDescriptor`.


## 5. Render App Registry & Resources

The `MaterialPlugin` initializes these core storage backends in the `RenderApp`:

* **`RenderPointCloudMaterialInstances`**: A type-erased resource map tracking which material asset is bound to which active point cloud entity for the current frame.
* **`SpecializedMaterialPipelineCache`**: The central cache holding the compiled GPU pipeline variants indexed by their `MaterialPipelineKey`.
* **`PendingMeshMaterialQueues`**: Tracks chunks and materials that are currently waiting for their pipelines to finish asynchronous compilation before they can be queued for rendering.
