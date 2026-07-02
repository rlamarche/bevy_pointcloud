# bevy_point_cloud

[![Crates.io](https://img.shields.io/crates/v/bevy_point_cloud.svg)](https://crates.io/crates/bevy_point_cloud)
[![Docs.rs](https://docs.rs/bevy_point_cloud/badge.svg)](https://docs.rs/bevy_point_cloud)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/your_username/bevy_point_cloud#license)

`bevy_point_cloud` is a high-performance, modular Bevy plugin dedicated to rendering massive (and standard) point clouds. 

Rather than being a standalone viewer application or a rigid, game-ready solution, this plugin is designed from the ground up as a **flexible framework**. It provides the essential building blocks required to integrate, manage, and render point cloud data seamlessly within the Bevy Engine.


## Core Features

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


## Architecture & Under the Hood

All features are built directly on top of the native **Bevy Engine rendering infrastructure**. There is no custom, hard-to-maintain rendering backend here: the plugin is a tailored adaptation of Bevy's mesh rendering pipelines, optimized specifically for point clouds and hierarchical batch rendering (which can also be adapted for voxels).

### Mesh Reuse & Advanced Shading Integration
Instead of reinventing the wheel, `bevy_point_cloud` piggybacks entirely on Bevy's native core types:
* **Chunks as Native Meshes:** A point cloud chunk is under the hood just a standard Bevy `Mesh`. This architectural choice allows the plugin to instantly leverage Bevy's optimized memory allocation mechanisms and internal slab allocations.
* **Flexible Point Meshes:** The underlying mesh drawn for each point within a chunk can be fully adapted. It can be configured for simple camera-facing billboarding or alternative behaviors.
* **PBR & Advanced Shading Support:** Because it relies on standard meshes, you can inject normals and tangents per point. This opens the door to advanced modern shading techniques, including standard Physically Based Rendering (PBR).

### A Note on Instanced Rendering in Bevy
It is worth noting that Bevy already handles standard mesh instancing automatically: when the engine detects the same mesh rendered multiple times with the same material instance, it batches them into a single instanced draw call.

**What `bevy_point_cloud` brings to the table** is a much finer level of control over this instancing process. It is specifically engineered to manage **millions or billions of instances organized in chunks**, hierarchically stored and streamed via an octree structure, which native mesh batching cannot handle alone.


## Getting Started

This section guides you through a minimal example to render a point cloud using `bevy_point_cloud`. 

To make prototyping and testing seamless, the plugin provides a `PointCloudMeshLoader` capable of converting native Bevy geometric meshes into point clouds. This allows you to quickly verify your setup and experiment with materials using standard shapes (like spheres or cubes) before loading massive external Potree data structures.

### Minimal Working Example

Here is the complete source code required to initialize the engine, setup the camera, and spawn a procedural point cloud sphere:

```rust
use bevy::prelude::*;
use bevy_pointcloud::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Initializes the core rendering architecture for point clouds
        .add_plugins(PointCloudPlugin::default())
        .add_systems(Startup, (setup, load_point_cloud))
        .run();
}

fn setup(mut commands: Commands) {
    // Basic 3D camera setup looking towards the origin
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(1.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn load_point_cloud(
    mut materials: ResMut<Assets<SimplePointCloudMaterial>>,
    point_cloud_server: Res<PointCloudServer>,
    mut commands: Commands,
) -> Result<()> {
    // 1. Generate a procedural mesh asset from Bevy and load it via the PointCloudServer
    let point_cloud_handle =
        point_cloud_server.load::<PointCloudMeshLoader>(Sphere::new(0.5).mesh().ico(16)?);

    // 2. Initialize a default instance of the point cloud material
    let material_handle = materials.add(SimplePointCloudMaterial::default());

    // 3. Spawn the point cloud entity with its spatial and material components
    commands.spawn((
        PointCloud3d(point_cloud_handle),
        PointCloudMaterial3d(material_handle),
    ));

    Ok(())
}
```

### Technical Deep Dive

While the framework handles the heavy lifting automatically, here is a quick look at how these core components interact to display your point cloud:

1. **The `PointCloudPlugin`**: Adding this plugin to your Bevy `App` initializes all the core rendering infrastructure. It registers the specialized render pipelines, uniform buffers, and custom shaders required to efficiently display and scale point primitives in 3D space.
2. **Loading Data via `PointCloudServer`**: The `PointCloudServer` is your gateway for importing point cloud resources. In this example, we use `PointCloudMeshLoader` to instantly transform a procedural Bevy `Mesh` (an Icosphere) into streamable point data. In a production environment, you will use this same server to asynchronously stream large-scale Potree octrees or `LAS`/`LAZ`/`COPC` files from your local assets or over the web.
3. **ECS Components (`PointCloud3d` & `PointCloudMaterial3d`)**: Following Bevy idioms, rendering a point cloud requires spawning an entity with two primary components:
   * `PointCloud3d`: Holds the asset `Handle` to your geometric point cloud data, telling the engine *what* data to render.
   * `PointCloudMaterial3d`: Links a custom material asset to your entity, telling the engine *how* the points should look.
4. **Customizing Visuals with `SimplePointCloudMaterial`**: The `SimplePointCloudMaterial` is a built-in asset used to control the shading and appearance of your points. By modifying this material in the Bevy `Assets` storage, you can easily customize properties such as point size scaling, point shape (square or circular), and color modes (RGB, intensities, etc.).


## Loader Responsibilities & Pipeline Boundaries

To remain highly modular, `bevy_point_cloud` establishes a clear boundary between data loading and rendering management. The framework leaves specific domain-logic to the `PointCloudLoader` implementations, which are responsible for:
* **Attribute Selection:** Selecting and filtering which data attributes to load into memory (avoiding the overhead of loading unused point data).
* **Attribute Reconstruction:** Dynamically computing missing attributes using point-cloud algorithms or spatial data structures like k-d trees (e.g., generating missing normals or directional ambient occlusion factors per point).
* **On-the-Fly Octree Generation:** Building the octree hierarchy at runtime if the source file is not already pre-optimized for streaming (for instance, by leveraging crates like `copc-converter`).
* **Offline Web Caching:** Implementing caching strategies for web-based asset loading. This keeps requested chunks cached on disk (offline storage) via HTTP range queries, heavily reducing network load and allowing instantaneous re-loading when a chunk becomes visible again.


## WebGL, WebGPU & Web Workers Roadmap

The plugin is designed with broad platform compatibility in mind, specifically targeting the web ecosystem:
* **WebGL 2.0 Support:** To ensure the plugin runs everywhere today, the current architecture avoids advanced modern GPU features that would break WebGL 2.0 compatibility. This is the main reason visibility tracking is currently backed by a specialized texture rather than advanced buffer structures.
* **Future WebGPU Roadmap:** Once WebGPU reaches General Availability (GA) and wider adoption, this texture-based approach could be upgraded to use `StorageBuffer` (SSBO). This will completely bypass texture size limitations and significantly improve tracking efficiency on modern hardware while maintaining a fallback for WebGL 2.0.
* **Multi-threaded Web Parsing:** Future roadmap entries include shifting chunk decompression and data parsing out of the main UI thread. By leveraging Web Workers alongside modern WASM multithreading capabilities, processing will happen on a background thread pool to ensure a locked 60+ FPS experience during heavy streaming.


## Acknowledgements

This crate would not have been possible without two major pillars:
1. **The Bevy Maintainers & Community:** For building an incredibly powerful, modular, and forward-thinking rendering infrastructure and ECS engine. 
2. **Markus Schütz (Creator of Potree):** This work relies heavily on the groundbreaking research and implementation found in **Potree**. The octree structure, hierarchical traversal logic, adaptive point sizing algorithms, and Eye-Dome Lighting (EDL) shading techniques used in this plugin are deeply inspired by his phenomenal contributions to the point cloud rendering ecosystem.


## License

`bevy_point_cloud` is free, open-source and licensed under either

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
