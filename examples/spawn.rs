#![expect(missing_docs, reason = "Not all docs are written yet.")]

use std::f32::consts::TAU;

use bevy::{
    camera::ScalingMode,
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    color::palettes::css::{RED, YELLOW},
    prelude::*,
};
use bevy_pointcloud::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FreeCameraPlugin)
        .add_plugins(PointCloudPlugin::default())
        .add_systems(
            Startup,
            (
                setup,
                init_spawn_configuration,
                load_point_cloud.after(init_spawn_configuration),
            ),
        )
        .add_systems(PostUpdate, spawn_point_cloud)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-4.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        FreeCamera::default(),
    ));
}

#[derive(Resource)]
struct SpawnConfiguration {
    pub material_handle: Handle<SimplePointCloudMaterial>,
    pub point_cloud_handle: Handle<PointCloud>,
}

fn init_spawn_configuration(
    mut commands: Commands,
    mut materials: ResMut<Assets<SimplePointCloudMaterial>>,
    point_cloud_server: Res<PointCloudServer>,
    asset_server: Res<AssetServer>,
) -> Result {
    let texture_handle = asset_server.load("branding/bevy_icon.png");

    let material_handle = materials.add(SimplePointCloudMaterial {
        shape_radius: None,
        base_color: RED.into(),
        base_color_texture: Some(texture_handle.clone()),
        point_size_mode: PointSizeMode::WorldSpace,
        point_size: 0.02, // in meters
        ..default()
    });

    let point_cloud_handle =
        point_cloud_server.load::<PointCloudMeshLoader>(Sphere::new(0.5).mesh().ico(16)?);

    commands.insert_resource(SpawnConfiguration {
        material_handle,
        point_cloud_handle,
    });

    Ok(())
}

fn load_point_cloud(
    mut commands: Commands,
    spawn_configuration: Res<SpawnConfiguration>,
) -> Result {
    commands.spawn((
        PointCloud3d(spawn_configuration.point_cloud_handle.clone()),
        PointCloudMaterial3d(spawn_configuration.material_handle.clone()),
        Transform::from_translation(Vec3::new(0.0, -1.0, -1.0)),
    ));

    Ok(())
}

fn spawn_point_cloud(
    mut commands: Commands,
    point_cloud: Query<Entity, With<PointCloud3d>>,
    key_input: Res<ButtonInput<KeyCode>>,
    spawn_configuration: Res<SpawnConfiguration>,
) {
    if key_input.just_pressed(KeyCode::Space) {
        let current_count = point_cloud.iter().len();

        commands.spawn((
            PointCloud3d(spawn_configuration.point_cloud_handle.clone()),
            PointCloudMaterial3d(spawn_configuration.material_handle.clone()),
            Transform::from_translation(Vec3::new(0.0, 0.0, current_count as f32 + 1.0)),
        ));
    }
}
