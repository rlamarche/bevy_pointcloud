# Data Preparation Pipeline

The Data Preparation Pipeline bridges the Bevy Main World (where your ECS entities and logic reside) and the Render World. This pipeline ensures that only the visible elements of your massive point clouds are prepared, packed, and uploaded to the GPU each frame.

> ⚠️ **Note:** This section covers the extraction and transformation pipelines for geometry and instances. Material preparation and shading parameters are more complex and are covered separately in the next section.


## 1. Extraction Phase

Systems in this phase run during Bevy's `ExtractSchedule` inside the `RenderApp`. Their primary job is to safely copy data from the Main World into the Render World, filtering out any data that isn't needed for the current frame.

### `extract_visible_point_cloud_chunks`
* **Execution Order:** Runs immediately after Bevy's native `extract_cameras` system.
* **Core Logic:** This system extracts the visible point cloud chunk entities into the render world while preserving their structural hierarchy. Crucially, it also computes a view-specific `first_child_index` and `children_mask` for each active view.
* **Purpose:** This computation prepares the topological data needed to generate the GPU visibility texture later on, ensuring the shaders know exactly which level-of-detail (LOD) nodes are active for each camera.

### `extract_pointcloud_chunk_instances`
* **Core Logic:** This system populates the `RenderPointCloudChunkInstances` resource, creating a dedicated `RenderPointCloudChunkInstance` for every single visible `PointCloudChunk3d`. 
* **Data Optimization:** To maximize performance and eliminate overhead during the extraction frame, this system completely bypasses copying mesh or asset IDs. It strictly extracts only the lightweight data required for spatial positioning and rendering features:
  * Its Axis-Aligned Bounding Box (`Aabb`), essential for viewport features and fine culling.
  * Its global transforms (`Transform`), which are required to position the chunk correctly in 3D space.


## 2. Preparation Phase

Once the required data is safely extracted into the Render World, it enters the preparation phase to be packed into GPU-friendly structures (such as Uniform Buffers) before rendering.

### `prepare_point_cloud_uniforms`
* **Core Logic:** This system processes the extracted transforms and attributes to build the uniform data for each point cloud instance, storing the results in the `PreparedPointCloudUniforms` resource.
* **Optimization Note:** Currently, **all chunks belonging to the same point cloud instance share a single, global point cloud uniform**. 
* **Why it matters:** Grouping chunks under a single uniform drastically reduces the number of bind group updates and binding switches required during the rendering phase, leading to significantly better CPU-to-GPU performance and smoother frame rates.
