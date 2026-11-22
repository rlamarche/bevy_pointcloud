#![expect(missing_docs, reason = "Not all docs are written yet.")]

use bevy::{
    color::palettes::css::{BLUE, GREEN, RED, YELLOW},
    core_pipeline::oit::{
        OrderIndependentTransparencyPlugin, OrderIndependentTransparencySettings,
    },
    prelude::*,
};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_pointcloud::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(PanOrbitCameraPlugin)
        .add_plugins(PointCloudPlugin::default())
        .add_systems(Startup, (setup, spawn_point_clouds))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
        PanOrbitCamera::default(),
        PointCloudViewSettings {
            render_modes: RenderModes::empty(),
            edl_enabled: false,
            ..default()
        },
        Msaa::Off,
        OrderIndependentTransparencySettings {
            sorted_fragment_max_count: 16,
            fragments_per_pixel_average: 8.0,
            ..default()
        },
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: light_consts::lux::OVERCAST_DAY,
            ..default()
        },
        Transform::from_xyz(2.0, 4.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn spawn_point_clouds(
    mut commands: Commands,
    point_cloud_server: Res<PointCloudServer>,
    mut simple_materials: ResMut<Assets<SimplePointCloudMaterial>>,
    mut std_materials: ResMut<Assets<StandardPointCloudMaterial>>,
) -> Result<()> {
    let sphere_cloud =
        point_cloud_server.load::<PointCloudMeshLoader>(Sphere::new(0.5).mesh().ico(8)?);

    // 1. SimplePointCloudMaterial - Rouge (phase transparente)
    let mat_red = simple_materials.add(SimplePointCloudMaterial {
        base_color: Color::from(RED).with_alpha(0.6),
        alpha_mode: AlphaMode::Blend,
        weighted_splats: false,
        ..default()
    });

    // 2. SimplePointCloudMaterial - Vert (phase transparente)
    let mat_green = simple_materials.add(SimplePointCloudMaterial {
        base_color: Color::from(GREEN).with_alpha(0.6),
        alpha_mode: AlphaMode::Blend,
        weighted_splats: false,
        ..default()
    });

    // 3. StandardPointCloudMaterial - Bleu (phase transparente)
    let mut mat_blue: StandardPointCloudMaterial = Color::from(BLUE).with_alpha(0.6).into();
    mat_blue.alpha_mode = AlphaMode::Blend;
    mat_blue.weighted_splats = false;
    let mat_blue_handle = std_materials.add(mat_blue);

    // 4. StandardPointCloudMaterial - Jaune (phase transparente)
    let mut mat_yellow: StandardPointCloudMaterial = Color::from(YELLOW).with_alpha(0.6).into();
    mat_yellow.alpha_mode = AlphaMode::Blend;
    mat_yellow.weighted_splats = false;
    let mat_yellow_handle = std_materials.add(mat_yellow);

    let splat_settings = SplatSettings {
        point_size_mode: PointSizeMode::LocalSpace,
        point_size: 0.04,
        radius: Some(0.5),
        orientation: SplatOrientation::FaceNormal,
        ..default()
    };

    commands.spawn((
        PointCloud3d(sphere_cloud.clone()),
        PointCloudMaterial3d(mat_red),
        splat_settings.clone(),
        Transform::from_xyz(-1.5, 0.0, 0.0),
    ));

    commands.spawn((
        PointCloud3d(sphere_cloud.clone()),
        PointCloudMaterial3d(mat_green),
        splat_settings.clone(),
        Transform::from_xyz(-0.5, 0.0, 0.0),
    ));

    commands.spawn((
        PointCloud3d(sphere_cloud.clone()),
        PointCloudMaterial3d(mat_blue_handle),
        splat_settings.clone(),
        Transform::from_xyz(0.5, 0.0, 0.0),
    ));

    commands.spawn((
        PointCloud3d(sphere_cloud),
        PointCloudMaterial3d(mat_yellow_handle),
        splat_settings,
        Transform::from_xyz(1.5, 0.0, 0.0),
    ));

    Ok(())
}
