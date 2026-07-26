#![expect(missing_docs, reason = "Not all docs are written yet.")]

use std::f32::consts::PI;

use bevy::{
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    color::palettes::css::{GREEN, RED},
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig},
    light::CascadeShadowConfigBuilder,
    prelude::*,
};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_pointcloud::{prelude::*, SplatOrientation, SplatSettings, UVMapping};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FpsOverlayPlugin {
            config: FpsOverlayConfig {
                text_config: TextFont {
                    // Here we define size of our overlay
                    font_size: FontSize::Px(42.0),
                    // If we want, we can use a custom font
                    font: default(),
                    // We could also disable font smoothing,
                    font_smoothing: FontSmoothing::default(),
                    ..default()
                },
                // We can also change color of the overlay
                text_color: GREEN.into(),
                // We can also set the refresh interval for the FPS counter
                refresh_interval: core::time::Duration::from_millis(100),
                enabled: true,
                frame_time_graph_config: FrameTimeGraphConfig {
                    enabled: true,
                    // The minimum acceptable fps
                    min_fps: 30.0,
                    // The target fps
                    target_fps: 144.0,
                },
            },
        })
        .add_plugins(FreeCameraPlugin)
        .add_plugins(PanOrbitCameraPlugin)
        // Initializes the core rendering architecture for point clouds
        .add_plugins(PointCloudPlugin::default())
        .add_systems(Startup, (setup, load_point_cloud))
        .add_systems(Update, toggle_splat_radius)
        // .add_systems(PostUpdate, draw_gizmos.after(propagate_parent_transforms))
        .run();
}

/// Toggles the `radius` property in `SplatSettings` when pressing the Space key.
fn toggle_splat_radius(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut SplatSettings>,
) {
    if keyboard_input.just_pressed(KeyCode::Space) {
        for mut splat_settings in &mut query {
            splat_settings.radius = match splat_settings.radius {
                Some(_) => None,
                None => Some(0.5),
            };
        }
    }
}

fn setup(mut commands: Commands) {
    // Basic 3D camera setup looking towards the origin
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(1.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        PanOrbitCamera::default(),
        FreeCamera::default(),
    ));
}

fn load_point_cloud(
    mut std_materials: ResMut<Assets<StandardPointCloudMaterial>>,
    point_cloud_server: Res<PointCloudServer>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) -> Result<()> {
    let texture_handle = asset_server.load("branding/bevy_icon.png");

    // 1. Generate a procedural mesh asset from Bevy and load it via the PointCloudServer
    let point_cloud_handle =
        point_cloud_server.load::<PointCloudMeshLoader>(Sphere::new(0.5).mesh().ico(8)?);

    let mut pbr_material: StandardPointCloudMaterial = Color::from(RED).into();
    pbr_material.cull_mode = None;
    pbr_material.double_sided = true;
    pbr_material.base_color_texture = Some(texture_handle);

    let pbr_material_handle = std_materials.add(pbr_material);

    commands.spawn((
        PointCloud3d(point_cloud_handle),
        PointCloudMaterial3d(pbr_material_handle.clone()),
        // PointCloudMaterial3d(material_handle),
        SplatSettings {
            orientation: SplatOrientation::FaceNormal,
            point_size_mode: PointSizeMode::LocalSpace,
            point_size: 0.05,
            radius: Some(0.5),
            uv_mapping: UVMapping::Planar,
            uv_u: Vec3::new(0.0, 0.0, 1.0),
            uv_v: Vec3::new(0.0, 1.0, 0.0),
            // min_point_size: Some(10.0),
            // max_point_size: Some(30.0),
            ..default()
        },
    ));

    // directional 'sun' light
    commands.spawn((
        DirectionalLight {
            illuminance: light_consts::lux::OVERCAST_DAY,
            shadow_maps_enabled: true,
            ..default()
        },
        // Transform::from_rotation(Quat::from_rotation_x(-PI / 2.0)),
        Transform {
            translation: Vec3::new(0.0, 2.0, 0.0),
            rotation: Quat::from_rotation_x(-PI / 2.),
            ..default()
        },
        // The default cascade config is designed to handle large scenes.
        // As this example has a much smaller world, we can tighten the shadow
        // bounds for better visual quality.
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 1.0,
            maximum_distance: 10.0,
            ..default()
        }
        .build(),
    ));

    Ok(())
}
