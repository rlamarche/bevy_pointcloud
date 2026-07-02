# GPU Preparation & Render Queue

After material extraction and pipeline specialization, the rendering pipeline enters its final stage. Here, material GPU resources are prepared, visible point cloud chunks are inserted into Bevy's render phases, and the draw commands assemble the complete GPU state before issuing instanced draw calls.


## 1. Material GPU Preparation

Material assets prepared during extraction still need their GPU bindings to be finalized before they can be rendered. This work occurs during **`RenderSystems::PrepareBindGroups`**.

### `prepare_material_bind_groups`
* **Responsibility:** Finalizing every material bind group required by the current frame.
* **Core Logic:** Iterates over all `MaterialBindGroupAllocator`s and allocates (or recreates) GPU bind groups for materials whose bindings have changed.
* **Output:** Every prepared material receives a stable `MaterialBindingId` that can later be resolved into an actual GPU bind group during rendering.
* **Optimization:** Allocators reuse existing GPU resources whenever possible, avoiding unnecessary bind group creation.

### `write_material_bind_group_buffers`
* **Responsibility:** Uploading allocator-managed GPU buffers.
* **Core Logic:** Flushes pending buffer updates through the render queue.
* **Why it matters:** This primarily affects **bindless materials**, where descriptor tables and backing storage must be synchronized before rendering begins.


## 2. Queueing Renderable Chunks

Once specialized pipelines and bind groups are available, every visible point cloud chunk is scheduled for rendering.

### `queue_material_meshes`
* **Scheduling:** Runs during **`RenderSystems::QueueMeshes`**.
* **Responsibility:** Assigning every visible chunk to its appropriate render phase.
* **Core Logic:**
  * Retrieves the specialized pipeline cached for the point cloud.
  * Resolves the associated material instance and prepared GPU resources.
  * Fetches the mesh allocation used for the chunk instance.
  * Inserts the chunk into the render phase matching the material's rendering mode.
* **Supported Render Phases:**
  * **Opaque**
  * **Alpha Mask**
  * **Transparent**
  * **Transmissive**

### Batching

For opaque rendering, compatible chunks are grouped using Bevy's native batching keys.

The batch key combines:
* the specialized pipeline,
* the material bind group,
* the mesh allocator slabs,
* optional lightmap information.

This minimizes render state changes while preserving correct material bindings.


## 3. Draw Commands

Queued render items reference one of the plugin's draw commands.

The default forward rendering path is:

```d2
vars: {
    d2-config: {
        pad: 50
    }
}

SetItemPipeline

SetMeshViewBindGroup

SetMeshViewBindingArrayBindGroup

SetPointCloudUniformGroup

SetMaterialBindGroup

DrawPointCloudInstanced

SetItemPipeline -> SetMeshViewBindGroup
SetMeshViewBindGroup -> SetMeshViewBindingArrayBindGroup
SetMeshViewBindingArrayBindGroup -> SetPointCloudUniformGroup
SetPointCloudUniformGroup -> SetMaterialBindGroup
SetMaterialBindGroup -> DrawPointCloudInstanced
```

Each command contributes one part of the GPU state before the final instanced draw call.


### `SetPointCloudUniformGroup`

* **Responsibility:** Binding the transform uniform associated with the point cloud instance.
* **Core Logic:** The command retrieves the current `RenderPointCloudChunkInstance`, follows its `root_entity`, and resolves the corresponding entry inside `PreparedPointCloudUniforms`.
* **Architectural Benefit:** Every octree chunk belonging to the same point cloud shares a single uniform bind group, keeping descriptor bindings constant across all chunk draw calls.


### `SetMaterialBindGroup`

* **Responsibility:** Binding material-specific GPU resources.
* **Core Logic:** Resolves the material assigned to the current point cloud entity through `RenderPointCloudMaterialInstances`, retrieves its allocated bind group, and binds it into the render pass.
* **Architectural Benefit:** Rendering remains fully type-erased while allowing arbitrary material implementations to provide their own shaders and GPU resources.


### `DrawPointCloudInstanced`

* **Responsibility:** Issuing the actual GPU draw call.
* **Core Logic:**
  * Retrieves the **shape mesh** defined by the material.
  * Retrieves the **point cloud chunk mesh** containing the per-point instance data.
  * Binds both vertex buffers:
    * **Slot 0:** Shape geometry (quad, sphere, cylinder, ...)
    * **Slot 1:** Point cloud instance data.
  * If the shape mesh is indexed, binds its index buffer before issuing an indexed instanced draw.
  * Otherwise, performs a non-indexed instanced draw.

This dual-buffer rendering model allows millions of points to share a lightweight primitive while every point contributes its own position and attributes through hardware instancing.


## 4. Pending Queues

Pipeline specialization and asset preparation are asynchronous. A visible point cloud may therefore become renderable before all of its GPU resources are available.

To handle this situation, the renderer maintains a **`PendingMeshMaterialQueues`** resource.

Whenever specialization or queueing cannot complete because a required resource is missing, the corresponding point cloud is stored inside the pending queue for its view.

Typical causes include:
* material asset still loading,
* render mesh not yet prepared,
* shape mesh unavailable,
* specialized pipeline still compiling.

Pending entries are retried automatically every frame until all required GPU resources become available.


## Render Pipeline Overview

```d2
vars: {
    d2-config: {
        pad: 50
    }
}

PreparedMaterial: "PreparedMaterial"
BindGroups: "Material Bind Groups"
PipelineCache: "Specialized Pipeline Cache"

Queue: "queue_material_meshes"

Draw: "Draw Commands"

GPU: "GPU Draw Call"

PreparedMaterial -> BindGroups: "prepare_material_bind_groups"
BindGroups -> Queue
PipelineCache -> Queue

Queue -> Draw
Draw -> GPU
```
