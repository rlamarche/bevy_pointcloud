# Loader Responsibilities & Pipeline Boundaries

To remain highly modular, `bevy_point_cloud` establishes a clear boundary between data loading and rendering management. The framework leaves the specific domain-logic to the `PointCloudLoader` implementations, which are responsible for:
* **Attribute Selection:** Selecting and filtering which data attributes to load into memory (avoiding the overhead of loading unused point data).
* **Attribute Reconstruction:** Dynamically computing missing attributes using point-cloud algorithms or spatial data structures like k-d trees (e.g., generating missing normals or directional ambient occlusion factors per point).
* **On-the-Fly Octree Generation:** Building the octree hierarchy at runtime if the source file is not already pre-optimized for streaming (for instance, by leveraging crates like `copc-converter`).
* **Offline Web Caching:** Implementing caching strategies for web-based asset loading. This keeps requested chunks cached on disk (offline storage) via HTTP range queries, heavily reducing network load and allowing instantaneous re-loading when a chunk becomes visible again.
