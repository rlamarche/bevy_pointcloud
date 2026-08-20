# Bevy Point Cloud

[![Crates.io](https://img.shields.io/crates/v/bevy_point_cloud.svg)](https://crates.io/crates/bevy_point_cloud)
[![Docs.rs](https://docs.rs/bevy_point_cloud/badge.svg)](https://docs.rs/bevy_point_cloud)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/your_username/bevy_point_cloud#license)

`bevy_pointcloud` is a high-performance, modular Bevy plugin dedicated to rendering massive (and standard) point clouds. 

Rather than being a standalone viewer application or a rigid, game-ready solution, this plugin is designed from the ground up as a **flexible framework**. It provides the essential building blocks required to integrate, manage, and render point cloud data seamlessly within the Bevy Engine.

## Acknowledgements

This crate would not have been possible without two major pillars:
1. **The Bevy Maintainers & Community:** For building an incredibly powerful, modular, and forward-thinking rendering infrastructure and ECS engine. 
2. **Markus Schütz (Creator of Potree):** This work relies heavily on the groundbreaking research and implementation found in **Potree**. The octree structure, hierarchical traversal logic, adaptive point sizing algorithms, and Eye-Dome Lighting (EDL) shading techniques used in this plugin are deeply inspired by his phenomenal contributions to the point cloud rendering ecosystem.


## License

`bevy_point_cloud` is free, open-source and licensed under either

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
