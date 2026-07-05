#![expect(missing_docs, reason = "Not all docs are written yet.")]

mod utils;

use bevy::{prelude::*, transform::systems::propagate_parent_transforms};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_pointcloud::prelude::*;

use crate::utils::draw_gizmos;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(PanOrbitCameraPlugin)
        // Initializes the core rendering architecture for point clouds
        .add_plugins(PointCloudPlugin::default())
        .add_systems(Startup, (setup, load_point_cloud))
        .add_systems(PostUpdate, draw_gizmos.after(propagate_parent_transforms))
        .run();
}

fn setup(mut commands: Commands) {
    // Basic 3D camera setup looking towards the origin
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(1.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        PanOrbitCamera::default(),
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
