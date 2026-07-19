#![expect(missing_docs, reason = "Not all docs are written yet.")]

mod utils;

use std::f32::consts::PI;

use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    camera::primitives::Aabb,
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    color::palettes::css::{RED, SILVER, YELLOW},
    core_pipeline::tonemapping::Tonemapping,
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig},
    light::{
        atmosphere::ScatteringMedium, cluster::ClusterConfig, Atmosphere,
        AtmosphereEnvironmentMapLight, CascadeShadowConfigBuilder, SunDisk, VolumetricFog,
    },
    math::VectorSpace,
    pbr::{AtmosphereSettings, ScreenSpaceReflections},
    post_process::bloom::Bloom,
    prelude::*,
    render::view::NoIndirectDrawing,
};
use bevy_pointcloud::{prelude::*, CopcLoader};

use crate::utils::draw_gizmos;

// --- RESOURCES AND STRUCTURES ---

struct OverlayColor;

impl OverlayColor {
    const RED: Color = Color::srgb(1.0, 0.0, 0.0);
    const GREEN: Color = Color::srgb(0.0, 1.0, 0.0);
}

#[derive(Resource)]
struct DayCycle {
    /// Day progression from 0.0 (midnight) to 1.0 (next midnight).
    progress: f32,
    /// Base speed factor for the daytime.
    base_speed: f32,
    /// Controls whether the sun is moving or paused.
    active: bool,
}

impl Default for DayCycle {
    fn default() -> Self {
        Self {
            progress: 0.25, // Start at dawn (0.25 * 2pi = 90°)
            base_speed: 0.02,
            active: true, // Active by default
        }
    }
}

#[derive(Component)]
struct CopcPointCloud;

// --- MAIN FUNCTION ---

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FpsOverlayPlugin {
            config: FpsOverlayConfig {
                text_config: TextFont {
                    font_size: FontSize::Px(42.0),
                    font: default(),
                    font_smoothing: FontSmoothing::default(),
                    ..default()
                },
                text_color: OverlayColor::GREEN,
                refresh_interval: core::time::Duration::from_millis(100),
                enabled: true,
                frame_time_graph_config: FrameTimeGraphConfig {
                    enabled: true,
                    min_fps: 30.0,
                    target_fps: 144.0,
                },
            },
        })
        .add_plugins(FreeCameraPlugin)
        .add_plugins(PointCloudPlugin::default())
        // Initialization and Resources
        .init_resource::<DayCycle>()
        // Startup Systems
        .add_systems(Startup, (setup, setup_sun, load_point_cloud))
        // Update Systems
        .add_systems(Update, (toggle_day_cycle, update_day_cycle))
        // PostUpdate Systems
        .add_systems(PostUpdate, center_point_cloud)
        .run();
}

// --- STARTUP SYSTEMS ---

fn setup(mut commands: Commands, mut scattering_mediums: ResMut<Assets<ScatteringMedium>>) {
    let earth_medium = scattering_mediums.add(ScatteringMedium::earth(256, 256));

    // Spawn earth atmosphere
    commands.spawn(Atmosphere::earth(earth_medium));

    // Basic 3D camera setup looking towards the origin
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(1.0, 1.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        FreeCamera::default(),
        PointCloudVisibilitySettings {
            min_radius: Some(30.0),
            point_budget: Some(10_000_000),
            ..default()
        },
        AtmosphereSettings::default(),
        Tonemapping::AcesFitted,
        Bloom::NATURAL, // Gives the sun a much more natural look
        AtmosphereEnvironmentMapLight::default(), /* Enables atmosphere to drive reflections and
                         * ambient lighting */
        VolumetricFog {
            ambient_intensity: 0.0,
            ..default()
        },
        Msaa::Off,
    ));
}

fn setup_sun(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            color: Color::WHITE,
            illuminance: light_consts::lux::AMBIENT_DAYLIGHT, // ~10,000 lux
            shadow_maps_enabled: true,
            ..default()
        },
        SunDisk::EARTH,
        CascadeShadowConfigBuilder {
            maximum_distance: 2000.0,
            first_cascade_far_bound: 10.0,
            ..default()
        }
        .build(),
        // Default orientation looks straight down (-Z axis)
        Transform::from_rotation(Quat::from_rotation_x(-PI / 4.0)),
        PointCloudVisibilitySettings {
            min_radius: Some(150.0),
            point_budget: Some(1_000_000),
            ..default()
        },
    ));
}

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
        "/home/romain/Documents/PointClouds/LidarHD/LHD_FXX_0893_6238_PTS_LAMB93_IGN69.copc.laz",
    )?);

    commands.spawn((
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        Visibility::default(),
        children![(
            CopcPointCloud,
            PointCloud3d(point_cloud_handle),
            PointCloudMaterial3d(material_handle),
        )],
    ));

    Ok(())
}

// --- UPDATE SYSTEMS ---

/// Toggles the sun movement when Spacebar is pressed
fn toggle_day_cycle(keyboard: Res<ButtonInput<KeyCode>>, mut day_cycle: ResMut<DayCycle>) {
    if keyboard.just_pressed(KeyCode::Space) {
        day_cycle.active = !day_cycle.active;
    }
}

fn update_day_cycle(
    time: Res<Time>,
    mut day_cycle: ResMut<DayCycle>,
    mut query: Query<(&mut Transform, &mut DirectionalLight)>,
) {
    // If paused, we do not advance time or update light positions
    if !day_cycle.active {
        return;
    }

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

// --- POST-UPDATE SYSTEMS ---

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
                z: 0.0, // Keep the original altitude
            }));
    }
}
