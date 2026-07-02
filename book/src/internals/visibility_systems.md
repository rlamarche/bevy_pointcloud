# Visibility Systems

This section details how `bevy_point_cloud` orchestrates its CPU-side management logic within Bevy's scheduling architecture.

## Visibility Calculation Pipeline

The plugin computes chunk visibility through a series of dedicated systems grouped into sequential `SystemSets`. This pipeline runs inside Bevy's **`PostUpdate`** schedule, integrating seamlessly with Bevy’s native visibility propagation layers (`ViewVisibility` and `VisibleEntities`).

### Overview of System Sets

The visibility pipeline is divided into three consecutive phases under the **`PointCloudVisibilitySystems`** enum:

```d2
vars: {
    d2-config: {
        pad: 50
    }
}

direction: down

VisibilitySystemsCheckVisibility: "VisibilitySystems::CheckVisibility (Bevy)"
VisibilitySystemsCheckVisibility.style.italic: true
VisibilitySystemsCheckVisibility.style.bold: false

TransformSystemsPropagate: "TransformSystems::Propagate (Bevy)"
TransformSystemsPropagate.style.italic: true
TransformSystemsPropagate.style.bold: false

PointCloudServerSystems: "PointCloudServerSystems"

CheckNodes: "PointCloudVisibilitySystems::CheckPointCloudNodesVisibility"
UpdateView: "PointCloudVisibilitySystems::UpdateViewVisibility"

VisibilitySystemsCheckVisibility -> CheckNodes
TransformSystemsPropagate -> CheckNodes
PointCloudServerSystems -> UpdateView
CheckNodes -> UpdateView
```

### `CheckPointCloudNodesVisibility`

This is the core algorithmic phase of the plugin, responsible for the global Screen Space Error (SSE) evaluation.

- **Responsibility:** Traverses the spatial hierarchies for each point cloud instance to find which precise chunks need to be rendered for each camera/view.
- **Core Logic:** For every active view (e.g., Cameras), and for each point cloud instance, a system traverses the Sparse Octree. It performs frustum culling through the hierarchy, reusing previous calculations if possible (when for example, a node is completly visible). It also computes Screen Space Error (SSE) - the screen pixel radius of the node in our case - metrics to determine LOD visibility.
- **Output:** Visible chunks are collected and populated into a **`VisiblePointCloudEntities`** component for each view. This phase operates globally, utilizing an optimized, pooled `BinaryHeap` allocation to evaluate priorities across all point cloud instances simultaneously.

### `UpdateViewVisibility`

The final phase that bridges `bevy_point_cloud` with Bevy's standard rendering loop.

- **Responsibility:** Injects the dynamically selected point cloud chunks into Bevy's native visibility pipeline for each view.
- **Core Logic:** This system reads the custom `VisiblePointCloudEntities` collection generated during the previous SSE evaluation phase. For each active camera view, it extracts the valid entity IDs of the visible octree nodes (`PointCloudChunk3d`) and appends them directly into Bevy's native **`VisibleEntities`** map (categorized under their specific `TypeId`). Concurrently, it updates the **`ViewVisibility`** component on each chunk entity to signal the engine that they are active for the current frame.
- **Why it matters:** By flushing your custom chunks straight into Bevy's standard `VisibleEntities` infrastructure, the rest of the engine's core extraction, batching, and render pipelines can process your streaming point cloud chunks like any other native 3D object—entirely bypassing the need for custom hacks or breaking engine conventions.

Note that this set runs after `PointCloudServerSystems` to be sure that loaded chunks are available.
