#![expect(missing_docs, reason = "Not all docs are written yet.")]

use std::f32::consts::PI;

use bevy::{
    camera::primitives::Aabb,
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    color::palettes::css::{GREEN, RED},
    core_pipeline::tonemapping::Tonemapping,
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig},
    image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    light::{
        atmosphere::ScatteringMedium, Atmosphere, AtmosphereEnvironmentMapLight,
        CascadeShadowConfigBuilder, SunDisk, VolumetricFog,
    },
    math::{Affine2, VectorSpace},
    pbr::AtmosphereSettings,
    platform::collections::HashSet,
    post_process::bloom::Bloom,
    prelude::*,
};
use bevy_pointcloud::{
    prelude::*, ClassificationExt, CopcLoader, FilterClassification, StandardPointCloudMaterial,
};
use las::point::Classification;

// --- RESOURCES AND STRUCTURES ---

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
                text_color: GREEN.into(),
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
        // Gives the sun a much more natural look
        Bloom::NATURAL,
        // Enables atmosphere to drive reflections and ambient lighting
        AtmosphereEnvironmentMapLight::default(),
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
            first_cascade_far_bound: 1.0,
            ..default()
        }
        .build(),
        // Default orientation looks straight down (-Z axis)
        Transform::from_rotation(Quat::from_rotation_x(-PI / 4.0)),
        PointCloudVisibilitySettings {
            min_radius: Some(30.0),
            point_budget: Some(1_000_000),
            ..default()
        },
    ));
}

fn load_point_cloud(
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut pc_standard_materials: ResMut<Assets<StandardPointCloudMaterial>>,
    point_cloud_server: Res<PointCloudServer>,
    mut commands: Commands,
) -> Result<()> {
    let texture_handle = asset_server
        .load_builder()
        .with_settings(|settings: &mut ImageLoaderSettings| {
            settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                ..default()
            });
        })
        .load("branding/bevy_icon.png");

    let textured_material_handle = pc_standard_materials.add(StandardMaterial {
        base_color_texture: Some(texture_handle),
        cull_mode: None,
        ..default()
    });

    let point_cloud_vegetation_handle = point_cloud_server.load_with_settings::<CopcLoader<_>>(FileSource::open(
        "/home/romain/Documents/PointClouds/LidarHD/LHD_FXX_0893_6238_PTS_LAMB93_IGN69.copc.laz",
    )?, |settings| {
        settings.filter_classification = FilterClassification::Include(HashSet::from([
            Classification::HighVegetation.as_u8(),
            Classification::LowVegetation.as_u8(),
        ]));
    });

    let point_cloud_other_handle = point_cloud_server.load_with_settings::<CopcLoader<_>>(FileSource::open(
        "/home/romain/Documents/PointClouds/LidarHD/LHD_FXX_0893_6238_PTS_LAMB93_IGN69.copc.laz",
    )?, |settings| {
        settings.filter_classification = FilterClassification::Exclude(HashSet::from([
            Classification::HighVegetation.as_u8(),
            Classification::LowVegetation.as_u8(),
        ]));
    });

    commands.spawn((
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        Visibility::default(),
        children![
            (
                CopcPointCloud,
                PointCloud3d(point_cloud_other_handle),
                PointCloudMaterial3d(
                    textured_material_handle /* pc_standard_materials.
                                              * add(StandardPointCloudMaterial(Color::from(RED).
                                              * into())) */
                ),
                SplatSettings {
                    splat: Some(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
                    point_size_mode: PointSizeMode::LocalSpace,
                    point_size: 0.5,
                    // radius: Some(0.5),
                    // orientation: SplatOrientation::Billboard,
                    orientation: SplatOrientation::FaceNormal,
                    default_normal: Vec3::new(0.0, 0.0, 1.0),
                    uv_mapping: UVMapping::Planar,
                    uv_u: Vec3::new(0.0, 1.0, 0.0),
                    uv_v: Vec3::new(1.0, 0.0, 0.0),
                    uv_transform: Affine2::from_scale_angle_translation(
                        Vec2 { x: 10.0, y: 10.0 },
                        0.0,
                        Vec2::ZERO,
                    ),
                    ..default()
                }
            ),
            (
                CopcPointCloud,
                PointCloud3d(point_cloud_vegetation_handle),
                PointCloudMaterial3d(
                    pc_standard_materials
                        .add(StandardPointCloudMaterial(Color::from(GREEN).into()))
                ),
                SplatSettings {
                    splat: Some(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
                    point_size_mode: PointSizeMode::LocalSpace,
                    point_size: 0.5,
                    // radius: Some(0.5),
                    // orientation: SplatOrientation::Billboard,
                    orientation: SplatOrientation::FaceNormal,
                    default_normal: Vec3::new(0.0, 0.0, 1.0),
                    // uv_mapping: UVMapping::Planar,
                    // uv_u: Vec3::new(0.0, 1.0, 0.0),
                    // uv_v: Vec3::new(1.0, 0.0, 0.0),
                    // uv_transform: Affine2::from_scale_angle_translation(
                    //     Vec2 { x: 10.0, y: 10.0 },
                    //     0.0,
                    //     Vec2::ZERO,
                    // ),
                    ..default()
                }
            )
        ],
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
