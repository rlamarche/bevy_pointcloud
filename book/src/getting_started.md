# Getting Started

This section guides you through a minimal example to render a point cloud using `bevy_pointcloud`. 

To make prototyping and testing seamless, the plugin provides a `PointCloudMeshLoader` capable of converting native Bevy geometric meshes into point clouds. This allows you to quickly verify your setup and experiment with materials using standard shapes (like spheres or cubes) before loading massive external Potree data structures.

## Minimal Working Example

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

---

## Technical Deep Dive

While the framework handles the heavy lifting automatically, here is a quick look at how these core components interact to display your point cloud:

### 1. The `PointCloudPlugin`
Adding this plugin to your Bevy `App` initializes all the core rendering infrastructure. It registers the specialized render pipelines, uniform buffers, and custom shaders required to efficiently display and scale point primitives in 3D space.

### 2. Loading Data via `PointCloudServer`
The `PointCloudServer` is your gateway for importing point cloud resources. 
* In this example, we use `PointCloudMeshLoader` to instantly transform a procedural Bevy `Mesh` (an Icosphere) into streamable point data.
* In a production environment, you will use this same server to asynchronously stream large-scale Potree octrees or `LAS`/`LAZ`/`COPC` files from your local assets or over the web.

### 3. ECS Components (`PointCloud3d` & `PointCloudMaterial3d`)
Following Bevy idioms, rendering a point cloud requires spawning an entity with two primary components:
* **`PointCloud3d`**: Holds the asset `Handle` to your geometric point cloud data, telling the engine *what* data to render.
* **`PointCloudMaterial3d`**: Links a custom material asset to your entity, telling the engine *how* the points should look.

### 4. Customizing Visuals with `SimplePointCloudMaterial`
The `SimplePointCloudMaterial` is a built-in asset used to control the shading and appearance of your points. By modifying this material in the Bevy `Assets` storage, you can easily customize properties such as:
* **Point Size & Scaling:** Toggle between fixed screen pixels or realistic world-space distance attenuation.
* **Point Shape:** Choose between square points or smooth circular points (via alpha discarding).
* **Color Modes:** Render points using their native RGB data, intensity values, or custom color ramps.
