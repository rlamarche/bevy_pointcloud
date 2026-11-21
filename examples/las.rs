#![expect(missing_docs, reason = "Not all docs are written yet.")]
use std::{borrow::Cow, f32::consts::PI};

use bevy::{
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    color::palettes::css::{BLUE, GREEN, RED},
    dev_tools::diagnostics_overlay::{DiagnosticsOverlay, DiagnosticsOverlayPlugin},
    diagnostic::{EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin},
    prelude::*,
};
use bevy_pointcloud::{las::LasLoaderSettings, prelude::*};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins((
            DiagnosticsOverlayPlugin,
            FrameTimeDiagnosticsPlugin::default(),
            EntityCountDiagnosticsPlugin::default(),
        ))
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
    mut simple_materials: ResMut<Assets<SimplePointCloudMaterial>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn(DiagnosticsOverlay {
        title: Cow::Owned("Statistics".to_owned()),
        items: vec![
            FrameTimeDiagnosticsPlugin::FPS.into(),
            FrameTimeDiagnosticsPlugin::FRAME_TIME.into(),
            EntityCountDiagnosticsPlugin::ENTITY_COUNT.into(),
        ],
    });

    let point_cloud = asset_server
        .load_builder()
        .with_settings::<LasLoaderSettings>(|s| {
            s.load_normals = true;
        })
        .load::<PointCloud>(
            // "pointclouds/lion_takanawa.copc.laz"
            "pointclouds/lion_takanawa_normals_curvature.copc.laz",
        );
    let material = materials.add(StandardPointCloudMaterial::default());
    let simple_material = simple_materials.add(SimplePointCloudMaterial::default());

    commands.spawn((
        PointCloud3d(point_cloud),
        PointCloudMaterial3d(material),
        SplatSettings {
            point_size_mode: PointSizeMode::LocalSpace,
            point_size: 0.025,
            radius: Some(0.5),
            // splat: Some(meshes.add(Cuboid::new(0.5, 0.5, 0.5))),
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
            color: Color::WHITE,
            illuminance: light_consts::lux::AMBIENT_DAYLIGHT, // ~10,000 lux
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_rotation_x(-PI / 4.0)),
    ));

    //     commands.spawn((
    //         DirectionalLight {
    //             color: RED.into(),
    //             illuminance: light_consts::lux::OVERCAST_DAY,
    //             shadow_maps_enabled: true,
    //             ..default()
    //         },
    //         Transform::from_xyz(10.0, 10.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
    //     ));
    //
    //     commands.spawn((
    //         DirectionalLight {
    //             color: GREEN.into(),
    //             illuminance: light_consts::lux::OVERCAST_DAY,
    //             shadow_maps_enabled: true,
    //             ..default()
    //         },
    //         Transform::from_xyz(0.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    //         PointCloudViewSettings::default(),
    //     ));
    //
    //     commands.spawn((
    //         DirectionalLight {
    //             color: BLUE.into(),
    //             illuminance: light_consts::lux::OVERCAST_DAY,
    //             shadow_maps_enabled: true,
    //             ..default()
    //         },
    //         Transform::from_xyz(10.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    //     ));
}
