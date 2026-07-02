# Core Features

By focusing on extensible building blocks, `bevy_point_cloud` handles the heavy lifting of data and memory management while giving you complete control over look and behavior:

### 1. Hierarchical Streamed Loaders (Octrees)
Implementing new streaming point cloud loaders is straightforward:
* **Simple Integration:** You only need to implement the `PointCloudLoader` trait.
* **Automated Lifecycle:** The plugin automatically handles the loading and unloading of octree nodes and chunks based on frustum visibility and your custom criteria (e.g., maximum point budget, minimum point size).
* **Memory Management:** CPU and GPU memory allocation, caching, and eviction are fully managed for you out of the box.
* **Progressive Web Streaming:** The framework natively supports progressive point cloud loading over the web using HTTP range queries.

### 2. Custom Material Shaders & Advanced Shading
The plugin reuses Bevy's powerful rendering logic while allowing you to fully customize visual output:
* **Vertex Shaders:** Override them to modify point shape, size, orientation, or procedural behavior.
* **Fragment Shaders:** Override them to implement your own custom shading, lighting, or classification techniques.
* **Configurable Materials:** Add any custom fields or uniforms to your material structs to pass data directly to your shaders.
* **Pipeline Caching:** Define your own pipeline keys to leverage Bevy's automatic shader compilation and render pipeline caching.
* **Texturing & UV Layouts:** The architecture supports applying a texture per individual point or globally across the entire point cloud by adapting how UV coordinates are calculated.

### 3. ECS-Friendly API
Managing your point cloud assets and instances is as intuitive as handling any other built-in Bevy asset:
* **First-Class Assets:** A point cloud is treated as a standard asset, where its loaded chunks act as sub-assets.
* **Native Transform Support & Data Sharing:** You can instantiate the same point cloud multiple times at different positions, rotations, and scales using the native `Transform` component. All instances share the same underlying loaded chunks, maximizing memory efficiency.
* **Automated Hierarchy Lifecycle:** Child chunk entities are automatically spawned and despawned in the ECS world during dynamic loading/unloading.
* **Strict ECS Relations:** The plugin preserves world hierarchy by leveraging Bevy's native `ChildOf` relation, introducing a specialized `ChildChunkOf` relation to maintain structural hierarchy specifically between the chunks themselves.

### 4. Efficient Screen Space Error Frustum Culling & LOD
Unlike standard renderable Bevy entities (such as `Mesh3d`), chunk visibility in `bevy_point_cloud` is accelerated directly through the octree structure:
* **Fast Traversal:** The octree data is packed and stored efficiently using a slotmap (slab) structure.
* **Bit-Efficient Indexing:** It utilizes bit-efficient `children_mask` and `child_index` layouts to provide ultra-fast hierarchical traversal.
* **Global SSE Prioritization:** Culling and Level-Of-Detail (LOD) decisions are computed based on screen space error (SSE) metrics. This prioritization is performed globally across all point cloud instances within a single visibility loop. 
* **Optimized Allocation:** The visibility loop leverages a `BinaryHeap` with pooled allocations that are reused across frames, minimizing runtime overhead and preventing CPU jitter.

### 5. Adaptive Point Size & GPU Instance Tracking
To achieve visually effective LOD rendering, the plugin maps the state of visible chunks directly to the GPU:
* **Visible Chunks Texture:** Based on the computed visibility, a specialized texture is constructed. For each point cloud octree instance, it stores the active visible chunks in a memory-locality-efficient structure using a dedicated `first_child_index` and instance-specific `children_mask`.
* **Dynamic Adaptive Point Size:** The scale of individual points within a chunk is computed dynamically inside the shader. It adapts automatically based on the visibility and LOD state of its respective sub-chunks, ensuring a visually seamless transition between different detail levels.
* **Current Capacity & Limits:** The default texture size is `2048x2048`. In the current implementation, this allows rendering up to `2048` unique octree instances, with a maximum of `2048` visible nodes *per* instance (where one row equals one instance).
* **Future Scalability:** These architectural limits are planned to be lifted in future updates:
  * The **2048 nodes-per-instance limit** will be removed by allowing a single point cloud instance to span across multiple rows (lines) within the same texture.
  * The **2048 total instances limit** will be removed by introducing support for multiple visibility textures.
