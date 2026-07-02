#![expect(missing_docs, reason = "Not all docs are written yet.")]

use bevy::prelude::*;
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
        Transform::from_xyz(1.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn load_point_cloud(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<SimplePointCloudMaterial>>,
    point_cloud_server: Res<PointCloudServer>,
) -> Result {
    let texture_handle = asset_server.load("branding/bevy_icon.png");
    commands.spawn((
        PointCloud3d(
            point_cloud_server.load::<PointCloudMeshLoader>(Sphere::new(0.5).mesh().ico(16)?),
        ),
        PointCloudMaterial3d(materials.add(SimplePointCloudMaterial {
            shape_radius: Some(0.5),
            base_color_texture: Some(texture_handle),
            point_size: 0.02,
            ..Default::default()
        })),
        Transform::from_scale(Vec3::new(1.0, 0.5, 1.0)),
    ));

    Ok(())
}
