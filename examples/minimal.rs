#![expect(missing_docs, reason = "Not all docs are written yet.")]

use bevy::{
    color::palettes::css::{GREEN, RED},
    prelude::*,
};
use bevy_pointcloud::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(PointCloudPlugin::default())
        .add_systems(Startup, (setup, load_point_cloud))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(15.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        // NoIndirectDrawing,
        // Msaa::Off,
    ));
}

fn load_point_cloud(
    mut materials: ResMut<Assets<StandardPointCloudMaterial>>,
    point_cloud_server: Res<PointCloudServer>,
    mut commands: Commands,
) -> Result {
    commands.spawn((
        PointCloud3d(point_cloud_server.load::<PointCloudMeshLoader>(
            Sphere::new(5.0).mesh().ico(16)?, // .uv(128, 72)
        )),
        PointCloudMaterial3d(materials.add(StandardPointCloudMaterial {
            base_color: RED.into(),
            base_color_up: Some(GREEN.into()),
            point_size: 100.0,
            ..default()
        })),
        Transform::from_scale(Vec3::new(1.0, 1.0, 2.0)),
    ));

    Ok(())
}
