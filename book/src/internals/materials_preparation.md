# Material Management: Extraction & Pipeline Specialization

To render point clouds with custom visual behaviors, `bevy_point_cloud` mirrors Bevy's type-erased material architecture. This page documents how materials are extracted from the main simulation world and how their render pipelines are specialized, highlighting our custom implementation of a **dual mesh vertex layout**.


## 1. Material Extraction & Systems Scheduling

During the **`ExtractSchedule`**, the plugin moves material data from the Main World into the Render World. The following systems run sequentially to handle this process:

### Extraction Systems & Order
* **`extract_mesh_materials::<M>`** (In `MaterialExtractionSystems`)
  Iterates over all entities holding a `PointCloudMaterial3d<M>` component. It fetches the material properties mapped via `AsBindGroup` (such as `AlphaMode`, `depth_bias`, shaders, and custom uniforms) and mirrors them into type-erased render representations.
* **`early_sweep_material_instances::<M>`** (Runs `.after(MaterialExtractionSystems).before(late_sweep_material_instances)`)
  Cleans up or prepares the raw instance mappings to synchronize with the current frame's extracted materials.
* **`extract_entities_needs_specialization::<M>`** (In `DirtySpecializationSystems::CheckForChanges`)
  Identifies which point cloud entities have dirty material states or new meshes requiring a pipeline re-evaluation.
* **`extract_entities_that_need_specializations_removed::<M>`** (In `DirtySpecializationSystems::CheckForRemovals`)
  Trashes stale specialization requests for entities or materials that no longer exist.


## 2. The `MaterialPipelineKey<M>`

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


## 3. Pipeline Specialization with Dual Layouts

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
  2. **The Instance Layout (`instance_layout`):** Represents the vertex structure of the *point cloud chunk data* (the heavy arrays of point positions, colors, or LIDAR intensities).
* **The Output:** These two layouts are mapped into **two distinct vertex buffer slots** (Slot 0 for the shape geometry at a per-vertex step rate, Slot 1 for the point data at a per-instance step rate) within the final `RenderPipelineDescriptor`.


## 4. Render App Registry & Resources

The `MaterialsPlugin` initializes these core storage backends in the `RenderApp`:

* **`RenderPointCloudMaterialInstances`**: A type-erased resource map tracking which material asset is bound to which active point cloud entity for the current frame.
* **`SpecializedMaterialPipelineCache`**: The central cache holding the compiled GPU pipeline variants indexed by their `MaterialPipelineKey`.
* **`PendingMeshMaterialQueues`**: Tracks chunks and materials that are currently waiting for their pipelines to finish asynchronous compilation before they can be queued for rendering.
