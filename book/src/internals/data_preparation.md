# Data Preparation Pipeline

This section details how `bevy_point_cloud` transfers data from the main world to the sub-render world, and how that data is massaged into GPU-ready structures before rendering.


## 1. Extraction Phase

Extraction is the bridge between your main ECS world and Bevy's isolated `RenderApp`. Systems in this phase run inside the **`ExtractSchedule`** and are strictly optimized to clone or compute only the minimal data required for the GPU.

### `extract_visible_point_cloud_chunks`
* **Scheduling:** Runs immediately after Bevy's native `extract_cameras` system.
* **Responsibility:** Mirroring visibility and structural states into the render world.
* **Core Logic:** This system extracts the visible point cloud chunk entities while preserving their logical layout. Crucially, it evaluates the visible topology per camera view to compute view-specific `first_child_index` and `children_mask` properties.
* **Why it matters:** This layout tracking is the data foundation used later in the pipeline to construct the specialized visibility texture mapping required for adaptive point sizing.

### `extract_pointcloud_chunk_instances`
* **Responsibility:** Flattening spatial and structural data into an optimized render cache.
* **Core Logic:** Iterates over every visible `PointCloudChunk3d` and populates the **`RenderPointCloudChunkInstances`** resource with lightweight `RenderPointCloudChunkInstance` metadata. 
* **Optimizations & Extracted Attributes:**
  * **`GlobalTransform`:** Extracted to build the instance-level uniform matrix.
  * **`Aabb`:** Extracted and cached. While frustum culling is already processed, storing the Axis-Aligned Bounding Box in the render world makes spatial boundaries immediately available for adaptive point sizing based on the LOD, advanced visual or debugging features directly inside the shaders.


## 2. Preparation Phase

Once data is safely inside the render world, the preparation phase packages raw components into GPU bind groups and uniform buffers. This phase runs inside Bevy's Render App schedules.

### `prepare_point_cloud_uniforms`
* **Responsibility:** Generating and packing global uniform data for point cloud instances.
* **Core Logic:** This system reads the extracted `GlobalTransform` matrices and compiles them into a **`PreparedPointCloudUniforms`** resource containing individual `PointCloudUniform` allocations.
* **Architectural Trade-off (Binding Efficiency):** > 💡 **Shared Instance Uniforms:** Currently, **all chunks belonging to the same point cloud instance share a single, global `PointCloudUniform`**. 
  > 
  > While assigning a dedicated uniform per individual octree node/chunk would allow for individual spatial adjustments, it would introduce catastrophic descriptor set re-bindings during draw calls. Sharing the transform uniform globally across the entire hierarchy dramatically reduces binding overhead and ensures optimal high-throughput rendering.



## Render-World Data Flow Overview

The diagram below summarizes how data progresses from main components to GPU-ready uniforms:

```d2
vars: {
    d2-config: {
        pad: 50
    }
}

direction: right

MainWorld: "Main World (ECS)" {
  Chunk: "PointCloudChunk3d"
  Root: "PointCloud3d"
}

RenderWorld: "Render World (RenderApp)" {
  Instances: "RenderPointCloudChunkInstances"
  Uniforms: "PreparedPointCloudUniforms"
}

MainWorld.Chunk -> RenderWorld.Instances: "extract_pointcloud_chunk_instances"
MainWorld.Root -> RenderWorld.Uniforms: "prepare_point_cloud_uniforms (via Transform)"
```
