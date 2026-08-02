#![expect(missing_docs, reason = "Not all docs are written yet.")]
use bevy::{
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    color::palettes::css::{BLUE, GREEN, RED},
    prelude::*,
};
use bevy_pointcloud::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FreeCameraPlugin)
        .add_plugins(PointCloudPlugin::default())
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardPointCloudMaterial>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
) {
    let point_cloud = asset_server.load::<PointCloud>("pointclouds/lion_takanawa.copc.laz");
    let material = materials.add(StandardPointCloudMaterial::default());

    commands.spawn((
        PointCloud3d(point_cloud),
        PointCloudMaterial3d(material),
        SplatSettings {
            point_size_mode: PointSizeMode::LocalSpace,
            point_size: 0.025,
            splat: Some(meshes.add(Cuboid::new(0.5, 0.5, 0.5))),
            orientation: SplatOrientation::FaceNormal,
            default_normal: Vec3::new(0.0, 0.0, 1.0),
            ..default()
        },
        Transform::from_rotation(Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2)),
    ));

    // --- GROUND PLANE FOR SHADOW RECEPTION ---
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(150.0, 150.0))),
        MeshMaterial3d(standard_materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.8,
            ..default()
        })),
        Transform::from_xyz(0.0, -2.5, 0.0),
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        FreeCamera::default(),
    ));

    commands.spawn((
        DirectionalLight {
            color: RED.into(),
            illuminance: light_consts::lux::OVERCAST_DAY,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 10.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        DirectionalLight {
            color: GREEN.into(),
            illuminance: light_consts::lux::OVERCAST_DAY,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        DirectionalLight {
            color: BLUE.into(),
            illuminance: light_consts::lux::OVERCAST_DAY,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
