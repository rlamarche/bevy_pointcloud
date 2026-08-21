#![expect(missing_docs, reason = "Not all docs are written yet.")]
mod ui;

use std::{f32::consts::PI, ops::Neg};

use bevy::{
    camera::primitives::Aabb,
    core_pipeline::tonemapping::Tonemapping,
    feathers::{dark_theme::create_dark_theme, theme::UiTheme, FeathersPlugins},
    light::{
        atmosphere::ScatteringMedium, Atmosphere, AtmosphereEnvironmentMapLight, SunDisk,
        VolumetricFog,
    },
    math::VectorSpace,
    pbr::AtmosphereSettings,
    post_process::bloom::Bloom,
    prelude::*,
};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_pointcloud::{
    potree::{PotreeAssetSource, PotreeLoader},
    prelude::*,
    HttpSource, StandardPointCloudMaterial,
};

use crate::ui::{MyUiPlugin, UiSettings, UiState};

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
struct MyPointCloud;

// --- MAIN FUNCTION ---

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins)
        .add_plugins(PanOrbitCameraPlugin)
        .add_plugins(FeathersPlugins)
        .add_plugins(MyUiPlugin)
        .add_plugins(PointCloudPlugin::default())
        // Initialization and Resources
        .init_resource::<DayCycle>()
        // Startup Systems
        .add_systems(Startup, (setup, setup_sun, load_point_cloud))
        // Update Systems
        .add_systems(Update, (toggle_day_cycle, update_day_cycle))
        // PostUpdate Systems
        .add_systems(PostUpdate, center_point_cloud)
        .add_systems(PreUpdate, update_camera_control);

    app.insert_resource(UiTheme(create_dark_theme()));

    app.run();
}

fn setup(mut commands: Commands, mut scattering_mediums: ResMut<Assets<ScatteringMedium>>) {
    let earth_medium = scattering_mediums.add(ScatteringMedium::earth(256, 256));

    // Spawn earth atmosphere
    commands.spawn(Atmosphere::earth(earth_medium));

    // Basic 3D camera setup looking towards the origin
    commands.spawn((
        Camera3d::default(),
        // Projection::Orthographic(OrthographicProjection {
        //     scale: 1.0,
        //     near: 0.0,
        //     far: 1000.0,
        //     viewport_origin: Vec2::new(0.5, 0.5),
        //     // scaling_mode: ScalingMode::WindowSize,
        //     scaling_mode: ScalingMode::FixedVertical {
        //         viewport_height: 20.0,
        //     },
        //     area: Rect::new(-1.0, -1.0, 1.0, 1.0),
        // }),
        Transform::from_xyz(20.0, 20.0, 0.0).looking_at(Vec3::new(0.0, 10.0, 0.0), Vec3::Y),
        PanOrbitCamera::default(),
        PointCloudVisibilitySettings {
            min_radius: Some(30.0),
            point_budget: Some(1_000_000),
            ..default()
        },
        AtmosphereSettings::default(),
        Tonemapping::AcesFitted,
        Bloom::NATURAL,
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
        Transform::from_rotation(Quat::from_rotation_x(-PI / 4.0)),
        PointCloudVisibilitySettings {
            min_radius: Some(30.0),
            point_budget: Some(1_000_000),
            ..default()
        },
    ));
}

fn load_point_cloud(
    mut meshes: ResMut<Assets<Mesh>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    mut pc_standard_materials: ResMut<Assets<StandardPointCloudMaterial>>,
    point_cloud_server: Res<PointCloudServer>,
    mut commands: Commands,
) -> Result<()> {
    let material_handle = pc_standard_materials.add(StandardMaterial {
        cull_mode: None,
        ..default()
    });

    // let point_cloud_handle =
    //     point_cloud_server.load::<PotreeLoader<_>>(PotreeAssetSource::<FileSource>::from_path(
    //         "assets/potree/heidentor",
    //     )?);

    let point_cloud_handle =
        point_cloud_server.load::<PotreeLoader<_>>(PotreeAssetSource::<HttpSource>::from_url(
            "https://pub-e2043f8abc6f45d983f8f77641ea772e.r2.dev/potree/heidentor",
        )?);

    commands.spawn((
        Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4)),
        Visibility::default(),
        children![(
            Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            Visibility::default(),
            children![(
                MyPointCloud,
                PointCloud3d(point_cloud_handle),
                PointCloudMaterial3d(material_handle),
                SplatSettings {
                    point_size_mode: PointSizeMode::LocalSpace,
                    radius: Some(0.5),
                    orientation: SplatOrientation::Billboard,
                    default_normal: Vec3::new(0.0, 0.0, 1.0),
                    ..default()
                }
            )],
        )],
    ));

    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(150.0, 150.0))),
        MeshMaterial3d(standard_materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.8,
            ..default()
        })),
        Transform::from_xyz(0.0, 1.8, 0.0),
    ));

    Ok(())
}

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

fn center_point_cloud(
    mut commands: Commands,
    mut camera: Query<
        (&mut Transform, &mut PanOrbitCamera),
        (With<Camera3d>, Without<PointCloud3d>),
    >,
    query: Query<(Entity, &Aabb), (With<PointCloud3d>, With<MyPointCloud>, Added<Aabb>)>,
) {
    let mut last_aabb = None;
    for (entity, aabb) in query {
        let center = aabb.center;
        commands
            .entity(entity)
            .insert(Transform::from_translation(Vec3 {
                x: -center.x,
                y: -center.y,
                z: 0.0, // Keep the original altitude
            }));

        last_aabb = Some(aabb);
    }
    // let Some((aabb, mut transform)) = query.iter_mut().next() else {
    //     return;
    // };

    // // Center point cloud
    // *transform = Transform::from_translation(
    //     (aabb.center.neg() + Vec3A::new(0.0, aabb.half_extents.y, 0.0)).into(),
    // );

    let Some(aabb) = last_aabb else {
        return;
    };

    let (camera_transform, mut pan_orbit_camera) = camera.single_mut().unwrap();

    let target_focus = Vec3::new(0.0, aabb.half_extents.y, 0.0);
    let (yaw, pitch, radius) = calculate_from_translation_and_focus(
        camera_transform.translation,
        target_focus,
        pan_orbit_camera.axis,
    );

    pan_orbit_camera.target_yaw = yaw;
    pan_orbit_camera.target_pitch = pitch;
    pan_orbit_camera.target_radius = radius;
    pan_orbit_camera.target_focus = target_focus;
}

fn calculate_from_translation_and_focus(
    translation: Vec3,
    focus: Vec3,
    axis: [Vec3; 3],
) -> (f32, f32, f32) {
    let axis = Mat3::from_cols(axis[0], axis[1], axis[2]);
    let comp_vec = translation - focus;
    let mut radius = comp_vec.length();
    if radius == 0.0 {
        radius = 0.05; // Radius 0 causes problems
    }
    let comp_vec = axis * comp_vec;
    let yaw = comp_vec.x.atan2(comp_vec.z);
    let pitch = (comp_vec.y / radius).asin();
    (yaw, pitch, radius)
}

fn update_camera_control(
    mut cameras: Query<
        (
            Option<&mut PanOrbitCamera>,
            &mut PointCloudVisibilitySettings,
        ),
        Without<DirectionalLight>,
    >,
    mut lights: Query<&mut PointCloudVisibilitySettings, With<DirectionalLight>>,
    ui_state: Res<UiState>,
    ui_settings: Res<UiSettings>,
) {
    for (pan_orbit_camera, mut visibility_settings) in &mut cameras {
        if let Some(mut pan_orbit_camera) = pan_orbit_camera {
            pan_orbit_camera.enabled = !ui_state.dragging && !ui_state.hovering;
        }
        visibility_settings.point_budget = Some(ui_settings.point_budget);
        visibility_settings.min_radius = Some(ui_settings.min_node_size);
    }
    for mut visibility_settings in &mut lights {
        visibility_settings.point_budget = Some(ui_settings.light_point_budget);
        visibility_settings.min_radius = Some(ui_settings.light_min_node_size);
    }
}
