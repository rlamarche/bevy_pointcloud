# Architecture Overview

## Core Definitions

```d2
vars: {
    d2-config: {
        pad: 50
    }
}

direction: left

PointCloud: "Point Cloud" {
  Octree: "Octree" {
    Node: "Node"
    Node <- Node: "Child of"
  }
  Octree <- Octree.Node: "Root of"
}


Chunk: "Chunk" {
  Points: "Points" {
    shape: cylinder
  }
}

Chunk <- PointCloud.Octree.Node: "Reference"
```

### Point Cloud

A **Point Cloud** is a data structure containing a collection of points, ranging from small datasets to massive, multi-million point arrays.

To efficiently manage and render these massive volumes of data, the points are organized into a spatially optimized hierarchical structure called an **Octree**.

### Octree

An **Octree** is a spatially optimized tree structure used for hierarchical data partitioning. It features a unique **root node** covering the entire bounding volume. Each node can be subdivided into up to 8 children, with each child representing an equally sized cubic subdivision of its parent's volume.

### Node

A **Node** represents an element within the Octree hierarchy. It keeps track of its parent and its 8 potential children slots.

Because `bevy_point_cloud` implements a **Sparse Octree**, nodes are only created for subdivisions that actually contain points.

A node is defined by the following properties:

- **`child_index: u8`**: Represents the node's local index within its parent's children array. By convention, this index is bit-encoded based on axis-aligned splits (`0bXYZ` format):

  A single `u8` is therefore more than sufficient to store these coordinates.
  - `0b000` -> `x = 0, y = 0, z = 0` (Lower-Left-Back)
  - `0b100` -> `x = 1, y = 0, z = 0` (Upper-Left-Back)
  - `0b010` -> `x = 0, y = 1, z = 0` (Lower-Right-Back)
  - `0b001` -> `x = 0, y = 0, z = 1` (Lower-Left-Front)
  - ...

- **`children_mask: u8`**: A bitmask indicating which of the 8 potential children actually exist. The bit offset corresponds to the decimal value of the `child_index`, calculated using the formula: `Index = (X * 4) + (Y * 2) + Z`.
- **`aabb: Option<Aabb>`**: The node's Axis-Aligned Bounding Box. It can be dynamically derived by subdividing the parent's AABB based on the node's `child_index`.
- **`depth: u32`**: The current depth level of the node within the octree hierarchy (where the root node is at depth `0`).

### Chunk

A **Chunk** holds the actual vertex data associated with a specific `Node`. While the Node manages the spatial hierarchy, the Chunk manages the rendering data payload.

In `bevy_point_cloud`, a Chunk is implemented as a Bevy `Asset` containing:

- **`mesh: Handle<Mesh>`**: A handle to the Bevy Mesh containing the point vertex data.
- **`aabb: Aabb`**: A copy of the corresponding node's bounding box, used for frustum culling.
- **`vertex_buffer_size: usize`**: The total size of the vertex buffer in bytes, providing immediate metrics on GPU memory usage.


### Point

A **Point** represents a single instance of an object or data position intended for mass rendering.
In `bevy_point_cloud`, each point is materialized as a single **vertex** within a Bevy `Mesh`.

- **Position**: Every point must have a 3D position attribute, which is strictly required for spatial rendering.
- **Attributes**: Points can optionally carry an arbitrary number of additional attributes depending on the use case:
  - **Color**: For RGB/RGBA point cloud visualization.
  - **Normal**: Used for PBR (Physically Based Rendering) shading or for orienting instanced meshes.
  - **LIDAR Data**: Industry-specific attributes such as intensity, return number, number of returns, classification flags, etc.


## Components and Assets

To interface with Bevy's ECS (Entity Component System) and asset management pipeline, `bevy_point_cloud` introduces a specific set of components and assets.

```d2
vars: {
    d2-config: {
        pad: 50
    }
}

direction: right

Assets: "Assets" {
  near: top-left
  PointCloud: "PointCloud"
  PointCloudChunk: "PointCloudChunk"
  PointCloud -> PointCloudChunk: References
}

Components: "Components" {
  near: top-right
  PointCloud3d: "PointCloud3d"
  PointCloudChunk3d: "PointCloudChunk3d"

  Transform: "Transform"
  Transform.style.bold: false
  Transform.style.italic: true
  
  PointCloud3d <- PointCloudChunk3d: ChildOf
  PointCloudChunk3d <- PointCloudChunk3d: ChildChunkOf
  PointCloud3d -> Transform: "Requires"
}

Assets.PointCloud <- Components.PointCloud3d: "Reference"
Assets.PointCloudChunk <- Components.PointCloudChunk3d: "Reference"
```

### Assets

Assets represent the heavy data payloads that are stored in memory and can be shared across multiple entities or systems.

- **`PointCloud`**: The master asset representing the whole point cloud structure. It holds the high-level octree topology and coordinates the internal hierarchy. It contains references (`Handle<PointCloudChunk>`) to all its constituent chunks.
- **`PointCloudChunk`**: The actual rendering data payload for a specific octree node. It contains the vertex buffer data (with a size tracked in bytes via `vertex_buffer_size`) and a `Handle<Mesh>` to allow Bevy to upload the data to the GPU.

### Components

Components are the building blocks attached to entities within the world to determine their behavior, transform, and visibility.

- **`PointCloud3d`**: The main component you attach to a root entity to spawn a point cloud in your 3D world. It acts as the anchor and refers to a `Handle<PointCloud>`.
- **`PointCloudMaterial3d<M>`**: A generic component attached to the root entity to define how the points should be shaded. `M` represents a custom material asset type (implementing Bevy's material traits), allowing for custom vertex coloring, shaders, or custom point rendering behaviors.
- **`PointCloudChunk3d`**: A component representing an active, renderable subset of the point cloud. It maps directly to a `PointCloudChunk` asset.

### Lifecycle and Hierarchy

When spawning a point cloud in your scene, you only need to manage the root entity:

```rust
// Example concept
commands.spawn((
    PointCloud3d { handle: point_cloud_handle },
    PointCloudMaterial3d { material: material_handle },
    Transform::from_xyz(0.0, 0.0, 0.0),
    Visibility::default(),
));
```

You do not need to manually spawn the `PointCloudChunk3d` components. `bevy_point_cloud` uses an automated system to manage the entity hierarchy:

1. **Lazy Loading**: As the camera moves or as the root `PointCloud` asset finishes loading, the internal management systems traverse the octree.
2. **Automatic Spawning**: The plugin automatically spawns child entities with the `PointCloudChunk3d` component for every chunk that needs to be streamed or rendered.
3. **Hierarchical Nesting**: All spawned `PointCloudChunk3d` entities have the root `PointCloud3d` entity as their direct ECS parent (via Bevy's `ChildOf` relationship), except for the root chunk, which **is** the `PointCloud3d` entity.

   This ensures they all share the same coordinate space and transform propagation without deep nesting overhead, as every chunk remains in the same frame of reference as the root.

   Instead, the custom `ChildChunkOf` relationship is strictly a logical pointer linking each chunk to its parent chunk, allowing you to easily query and traverse the internal chunk hierarchy from an ECS query. This setup guarantees that deleting or moving the root point cloud correctly cascades through all active chunks while keeping query access simple and efficient.


## Materials

When rendering point clouds, achieving precise visual control over points—whether for scientific visualization, aesthetic styling, or performance tuning—is essential. 

Instead of reinventing the wheel, `bevy_point_cloud` is built directly on top of **Bevy's native Material system**. By duplicating Bevy's core material paradigms and adapting them with custom vertex and fragment behaviors, the plugin blends seamlessly into the engine's existing asset and rendering architecture.

This architectural choice ensures that:
* **Materials are First-Class Assets:** They are managed by Bevy’s `AssetServer`, benefit from hot-reloading, and can be easily duplicated or modified at runtime.
* **Instance Sharing & Efficiency:** A single material asset instance can be shared and reused across multiple distinct point cloud instances in your scene. This dramatically reduces bind group re-bindings and maximizes GPU rendering efficiency.
* **Fully Extensible:** You can implement the `PointCloudMaterial` trait to provide your own custom WGSL shaders, bind groups, and uniform properties, leveraging Bevy's automatic pipeline caching under the hood.


### The Built-in `SimplePointCloudMaterial`

To get you started immediately without writing custom shaders, `bevy_point_cloud` provides a highly versatile, feature-rich default material asset named **`SimplePointCloudMaterial`**. It leverages both CPU-side properties and GPU-side dynamic evaluation to offer advanced styling out of the box:

* **Adaptive Point Sizing:** Dynamically scales points based on local LOD density to maintain a uniform visual density across the cloud. Instead of applying a single size per chunk, the shader scales individual points depending on which surrounding nodes of the octree are drawn. Points in areas where finer child chunks are missing are rendered larger to fill gaps, ensuring that all visible points within the same spatial zone maintain a consistent visual weight and seamless transitions.
* **Coloring & Texturing:**
  * **Base Color:** Apply a uniform color tint across the cloud.
  * **Global Texturing (Planned):** Map a global texture across the entire point cloud.
  * **Point Texturing:** Apply a texture or a custom mesh shape uniformly to all point primitives using point-level UV layouts. *(Note: Texture atlas swapping per-point is possible via custom shaders by leveraging point attributes).*
* **Advanced Color Gradients:** Built-in support for procedural color ramps featuring up to **8 customizable stops**. Gradients can be mapped dynamically based on:
  * **Directional Vectors & Bounds:** Map gradients along any axis (e.g., a vertical vector for height/Z-axis coloring) by providing custom min/max bounds directly in the material. This allows multiple point clouds to easily share the same coordinate scale.
  * **Custom Point Attributes (Planned):** Map gradients dynamically using any arbitrary point attribute sent to the GPU (such as LIDAR intensity, classification, or custom gameplay data).
* **Procedural & Custom Shapes:** Go beyond simple square pixels. Out of the box, the default shader can procedurally turn standard quads into **perfect disks** using a radius property. It also allows you to provide a custom base mesh to change the shape of all points globally.

```d2
vars: {
    d2-config: {
        pad: 50
    }
}

direction: right

Assets: "Assets" {
  near: top-left
  SimplePointCloudMaterial
}

Components: "Components" {
  near: top-right
  SimplePointCloudMaterial3d: "PointCloudMaterial3d<SimplePointCloudMaterial>"
}

Assets.SimplePointCloudMaterial <- Components.SimplePointCloudMaterial3d: "Reference"
```


### Integrating with the Pipeline

In practice, a material asset is bound to a point cloud root entity via the generic **`PointCloudMaterial3d<M>`** component (e.g., `PointCloudMaterial3d<SimplePointCloudMaterial>`). 

By attaching this component to your entity, you pass a `Handle<SimplePointCloudMaterial>` pointing to your material asset. This architecture allows multiple distinct entities, each with their own `PointCloud3d` component, to reference the exact same `SimplePointCloudMaterial` asset handle, ensuring optimal data reuse across the rendering pipeline.


```d2
vars: {
    d2-config: {
        pad: 50
    }
}

direction: right

PointClouds: "Point Clouds" {
  # near: top-left
  MyPointCloud1: "My Point Cloud 1"
  MyPointCloud2: "My Point Cloud 2"
}

SimpleMaterials: "Simple Materials" {
  # near: top-center
  MyMaterial: "My Material instance"
}

Entities: "Entities" {
  # near: bottom-center
  MyPointCloudInstance1: "My Point Cloud Instance 1" {
    PointCloud: "PointCloud3d"
    Material: "PointCloudMaterial3d<..>"
  }
  MyPointCloudInstance2: "My Point Cloud Instance 2" {
    PointCloud: "PointCloud3d"
    Material: "PointCloudMaterial3d<..>"
  }
}

PointClouds.MyPointCloud1 <- Entities.MyPointCloudInstance1.PointCloud: "Reference"
PointClouds.MyPointCloud2 <- Entities.MyPointCloudInstance2.PointCloud: "Reference"
SimpleMaterials.MyMaterial <- Entities.MyPointCloudInstance1.Material: "Reference"
SimpleMaterials.MyMaterial <- Entities.MyPointCloudInstance2.Material: "Reference"
```
