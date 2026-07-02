# Architecture & Under the Hood

All features are built directly on top of the native **Bevy Engine rendering infrastructure**. There is no custom, hard-to-maintain rendering backend here: the plugin is a tailored adaptation of Bevy's mesh rendering pipelines, optimized specifically for point clouds and hierarchical batch rendering (which can also be adapted for voxels).

### Mesh Reuse & Advanced Shading Integration
Instead of reinventing the wheel, `bevy_point_cloud` piggybacks entirely on Bevy's native core types:
* **Chunks as Native Meshes:** A point cloud chunk is under the hood just a standard Bevy `Mesh`. This architectural choice allows the plugin to instantly leverage Bevy's optimized memory allocation mechanisms and internal slab allocations.
* **Flexible Point Meshes:** The underlying mesh drawn for each point within a chunk can be fully adapted. It can be configured for simple camera-facing billboarding or alternative behaviors.
* **PBR & Advanced Shading Support:** Because it relies on standard meshes, you can inject normals and tangents per point. This opens the door to advanced modern shading techniques, including standard Physically Based Rendering (PBR).

### A Note on Instanced Rendering in Bevy
It is worth noting that Bevy already handles standard mesh instancing automatically: when the engine detects the same mesh rendered multiple times with the same material instance, it batches them into a single instanced draw call.

**What `bevy_point_cloud` brings to the table** is a much finer level of control over this instancing process. It is specifically engineered to manage **millions or billions of instances organized in chunks**, hierarchically stored and streamed via an octree structure, which native mesh batching cannot handle alone.
