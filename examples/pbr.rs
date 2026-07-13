#![expect(missing_docs, reason = "Not all docs are written yet.")]

mod utils;

use std::f32::consts::PI;

use bevy::{
    camera::primitives::Aabb,
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    color::palettes::css::{GREEN, RED, SILVER, YELLOW},
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig},
    light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap},
    math::VectorSpace,
    pbr::ScreenSpaceAmbientOcclusion,
    prelude::*,
    transform::systems::propagate_parent_transforms,
};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_pointcloud::{prelude::*, CopcLoader};

use crate::utils::draw_gizmos;

struct OverlayColor;

impl OverlayColor {
    const RED: Color = Color::srgb(1.0, 0.0, 0.0);
    const GREEN: Color = Color::srgb(0.0, 1.0, 0.0);
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // .add_plugins(FpsOverlayPlugin {
        //     config: FpsOverlayConfig {
        //         text_config: TextFont {
        //             // Here we define size of our overlay
        //             font_size: FontSize::Px(42.0),
        //             // If we want, we can use a custom font
        //             font: default(),
        //             // We could also disable font smoothing,
        //             font_smoothing: FontSmoothing::default(),
        //             ..default()
        //         },
        //         // We can also change color of the overlay
        //         text_color: OverlayColor::GREEN,
        //         // We can also set the refresh interval for the FPS counter
        //         refresh_interval: core::time::Duration::from_millis(100),
        //         enabled: true,
        //         frame_time_graph_config: FrameTimeGraphConfig {
        //             enabled: true,
        //             // The minimum acceptable fps
        //             min_fps: 30.0,
        //             // The target fps
        //             target_fps: 144.0,
        //         },
        //     },
        // })
        .add_plugins(FreeCameraPlugin)
        .add_plugins(PanOrbitCameraPlugin)
        // Initializes the core rendering architecture for point clouds
        .add_plugins(PointCloudPlugin::default())
        .add_systems(Startup, (setup, load_point_cloud))
        .add_systems(PostUpdate, center_point_cloud)
        .init_resource::<DayCycle>()
        .add_systems(Startup, setup_sun)
        .add_systems(Update, update_day_cycle)
        // .add_systems(PostUpdate, draw_gizmos.after(propagate_parent_transforms))
        .run();
}

#[derive(Resource)]
struct DayCycle {
    /// Day progression from 0.0 (midnight) to 1.0 (next midnight).
    progress: f32,
    /// Base speed factor for the daytime.
    base_speed: f32,
}

impl Default for DayCycle {
    fn default() -> Self {
        Self {
            progress: 0.25, // Start at dawn (0.25 * 2pi = 90°)
            base_speed: 0.02,
        }
    }
}

fn setup_sun(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            color: Color::WHITE,
            illuminance: light_consts::lux::AMBIENT_DAYLIGHT, // ~10,000 lux
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            maximum_distance: 2048.0,
            first_cascade_far_bound: 10.0,
            ..default()
        }
        .build(),
        // Default orientation looks straight down (-Z axis)
        Transform::from_rotation(Quat::from_rotation_x(-PI / 4.0)),
    ));
}

fn update_day_cycle(
    time: Res<Time>,
    mut day_cycle: ResMut<DayCycle>,
    mut query: Query<(&mut Transform, &mut DirectionalLight)>,
) {
    // Determine the current angle to see if it's day or night
    let current_angle = day_cycle.progress * 2.0 * PI;
    let is_night = current_angle.sin() <= 0.0;

    // Apply a 10x speed multiplier during the night
    let current_speed = if is_night {
        day_cycle.base_speed * 10.0
    } else {
        day_cycle.base_speed
    };

    // Advance time based on the contextual speed
    day_cycle.progress += current_speed * time.delta_secs();
    if day_cycle.progress > 1.0 {
        day_cycle.progress -= 1.0;
    }

    // Recalculate final angle and height after progression
    let angle = day_cycle.progress * 2.0 * PI;
    let sun_height = angle.sin();

    for (mut transform, mut light) in &mut query {
        // Rotate around X axis to simulate East -> West trajectory
        transform.rotation = Quat::from_rotation_x(-angle);

        if sun_height > 0.0 {
            // Daytime setup
            light.illuminance = sun_height * light_consts::lux::AMBIENT_DAYLIGHT;

            // Color interpolation: Red/Orange at dawn/dusk, White/Yellow at noon
            let t = sun_height.clamp(0.0, 1.0);
            let sunset_color = Color::linear_rgb(1.0, 0.4, 0.2);
            let noon_color = Color::linear_rgb(1.0, 1.0, 0.9);

            // Bevy 0.19 prefers linear color space blending
            light.color = Color::from(sunset_color.to_linear().lerp(noon_color.to_linear(), t));
        } else {
            // Nighttime setup (Moonlight or faint dark blue tint)
            light.illuminance = 0.1;
            light.color = Color::linear_rgb(0.1, 0.1, 0.2);
        }
    }
}

fn setup(
    mut commands: Commands,
    // mut directional_light_shadow_map: ResMut<DirectionalLightShadowMap>,
) {
    // directional_light_shadow_map.size = 4096;

    // Basic 3D camera setup looking towards the origin
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(1.0, 1.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        // PanOrbitCamera::default(),
        FreeCamera::default(),
        PointCloudVisibilitySettings {
            min_radius: Some(30.0),
            point_budget: Some(50_000_000),
            // max_depth: Some(3),
            ..default()
        },
        // ScreenSpaceAmbientOcclusion::default(),
        Msaa::Off,
    ));
}

#[derive(Component)]
struct CopcPointCloud;

fn load_point_cloud(
    mut materials: ResMut<Assets<StandardMaterial>>,
    point_cloud_server: Res<PointCloudServer>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
) -> Result<()> {
    let mut material: StandardMaterial = Color::from(RED).into();
    material.cull_mode = None;
    let material_handle = materials.add(material);

    let point_cloud_handle =
        point_cloud_server.load::<PointCloudMeshLoader>(Sphere::new(0.5).mesh().ico(16)?);

    commands.spawn((
        PointCloud3d(point_cloud_handle),
        PointCloudMaterial3d(material_handle.clone()),
        Transform::from_translation(Vec3::new(0.0, 1.0, 0.0)),
    ));

    // let sphere = meshes.add(Sphere::default().mesh().ico(5).unwrap());
    // commands.spawn((
    //     Mesh3d(sphere),
    //     MeshMaterial3d(materials.add(Color::from(GREEN))),
    //     Transform::from_translation(Vec3::new(0.0, 2.0, 1.0)),
    //     // NotShadowCaster,
    // ));

    commands.spawn((
        Mesh3d(
            meshes.add(
                Plane3d::default()
                    .mesh()
                    .size(500.0, 500.0)
                    .subdivisions(10),
            ),
        ),
        MeshMaterial3d(materials.add(Color::from(SILVER))),
    ));

    let point_cloud_handle = point_cloud_server.load::<CopcLoader<_>>(FileSource::open(
        // "assets/pointclouds/lion_takanawa.copc.laz",
        "/home/romain/Documents/PointClouds/LidarHD/LHD_FXX_0893_6238_PTS_LAMB93_IGN69.copc.laz",
    )?);

    commands.spawn((
        Transform::from_rotation(Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2)),
        children![(
            CopcPointCloud,
            PointCloud3d(point_cloud_handle),
            PointCloudMaterial3d(material_handle),
        )],
    ));

    Ok(())
}

fn center_point_cloud(
    loaded_point_clouds: Query<
        (Entity, &Aabb),
        (With<PointCloud3d>, With<CopcPointCloud>, Added<Aabb>),
    >,
    mut commands: Commands,
) {
    for (entity, aabb) in loaded_point_clouds {
        let center = aabb.center;
        commands
            .entity(entity)
            .insert(Transform::from_translation(Vec3 {
                x: -center.x,
                y: -center.y,
                z: 0.0, // to keep the altitude
            }));
    }
}
