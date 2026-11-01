#![recursion_limit = "256"]
#![expect(missing_docs, reason = "Not all docs are written yet.")]
mod ui;

use std::{borrow::Cow, f32::consts::PI, ops::Deref};

use bevy::{
    camera::primitives::Aabb,
    dev_tools::{
        diagnostics_overlay::{DiagnosticsOverlay, DiagnosticsOverlayPlugin},
        render_debug::{RenderDebugMode, RenderDebugOverlay},
    },
    diagnostic::{
        EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
        SystemInformationDiagnosticsPlugin,
    },
    ecs::template::OptionTemplate,
    feathers::{dark_theme::create_dark_theme, theme::UiTheme, FeathersPlugins},
    math::VectorSpace,
    pbr::diagnostic::MaterialAllocatorDiagnosticPlugin,
    prelude::*,
    render::{diagnostic::MeshAllocatorDiagnosticPlugin, view::NoIndirectDrawing},
    window::PresentMode,
};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_pointcloud::{
    potree::{PotreeAssetSource, PotreeLoader},
    prelude::*,
    render::SplatMeshes,
    PointSizeModeTemplate, RenderMode, RenderModes, SplatOrientationTemplate,
    StandardPointCloudMaterial,
};

use crate::ui::{
    MyUiPlugin, UiClickDespawnEvent, UiClickSetSplatOrientation, UiClickSpawnEvent, UiSettings,
    UiState,
};

// --- RESOURCES AND STRUCTURES ---

#[derive(Resource)]
struct SharedState {
    pub std_material: Handle<StandardPointCloudMaterial>,
    pub simple_material: Handle<SimplePointCloudMaterial>,
    pub point_cloud: Handle<PointCloud>,
    pub root_entity: Entity,
    pub parent_entity: Entity,
    pub triangle: Handle<Mesh>,
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
            active: false, // Active by default
        }
    }
}

#[derive(Component, FromTemplate)]
struct MyPointCloud;

// --- MAIN FUNCTION ---

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins)
        .add_plugins((
            FrameTimeDiagnosticsPlugin::default(),
            MeshAllocatorDiagnosticPlugin,
            EntityCountDiagnosticsPlugin::default(),
            MaterialAllocatorDiagnosticPlugin::<StandardMaterial>::default(),
            SystemInformationDiagnosticsPlugin,
            DiagnosticsOverlayPlugin,
        ))
        .add_plugins(PanOrbitCameraPlugin)
        .add_plugins(FeathersPlugins)
        .add_plugins(MyUiPlugin)
        .add_plugins(PointCloudPlugin::default())
        // Initialization and Resources
        .init_resource::<DayCycle>()
        // Startup Systems
        .add_systems(
            Startup,
            (
                setup_window,
                init_shared_state,
                setup_camera,
                setup_scene.after(init_shared_state),
                setup_sun,
            ),
        )
        // Update Systems
        .add_systems(Update, (toggle_day_cycle, update_day_cycle))
        // PostUpdate Systems
        .add_systems(PostUpdate, center_point_cloud)
        .add_systems(PreUpdate, update_camera_control)
        .add_observer(despawn_observer)
        .add_observer(spawn_observer)
        .add_observer(set_splat_orientation_observer);

    app.insert_resource(UiTheme(create_dark_theme()));

    app.run();
}

fn setup_window(mut windows: Query<&mut Window>) {
    let mut window = windows.single_mut().unwrap();
    window.present_mode = PresentMode::Mailbox;
}

fn init_shared_state(
    mut pc_standard_materials: ResMut<Assets<StandardPointCloudMaterial>>,
    mut pc_simple_materials: ResMut<Assets<SimplePointCloudMaterial>>,
    point_cloud_server: Res<PointCloudServer>,
    mut commands: Commands,
    splat_meshes: Res<SplatMeshes>,
) -> Result<()> {
    let std_material = pc_standard_materials.add(StandardMaterial {
        cull_mode: None,
        // opaque_render_method: bevy::material::OpaqueRendererMethod::Deferred,
        // alpha_mode: AlphaMode::Blend,
        ..default()
    });

    let simple_material = pc_simple_materials.add(SimplePointCloudMaterial {
        cull_mode: None,
        render_mode: RenderMode::Normal,
        weighted_splats: false,
        // opaque_render_method: bevy::material::OpaqueRendererMethod::Deferred,
        // alpha_mode: AlphaMode::Blend,
        ..default()
    });

    let root_entity = commands
        .spawn((
            Name::new("Root entity"),
            Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4)),
            Visibility::default(),
        ))
        .id();

    let parent_entity = commands
        .spawn((
            Name::new("Parent entity"),
            ChildOf(root_entity),
            Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            Visibility::default(),
        ))
        .id();

    // let point_cloud =
    //     point_cloud_server.load::<PotreeLoader<_>>(PotreeAssetSource::<HttpSource>::from_url(
    //         "https://pub-e2043f8abc6f45d983f8f77641ea772e.r2.dev/potree/heidentor",
    //     )?);

    let point_cloud = point_cloud_server.load::<PotreeLoader<_>>(
        PotreeAssetSource::<FileSource>::from_path("assets/potree/heidentor")?,
    );

    // let point_cloud = point_cloud_server.load::<LasLoader<_>>(FileSource::open(
    //     "assets/pointclouds/lion_takanawa.copc.laz",
    // )?);

    commands.insert_resource(SharedState {
        std_material,
        simple_material,
        point_cloud,
        root_entity,
        parent_entity,
        triangle: splat_meshes.triangle_mesh.clone(),
    });

    Ok(())
}

fn setup_camera(mut commands: Commands) {
    // commands.spawn(DiagnosticsOverlay::fps());
    // commands.spawn(DiagnosticsOverlay::mesh_and_standard_material());
    commands.spawn(DiagnosticsOverlay {
        title: Cow::Owned("Statistics".to_owned()),
        items: vec![
            FrameTimeDiagnosticsPlugin::FPS.into(),
            FrameTimeDiagnosticsPlugin::FRAME_TIME.into(),
            EntityCountDiagnosticsPlugin::ENTITY_COUNT.into(),
            // DiagnosticsOverlayItem {
            //     path: FrameTimeDiagnosticsPlugin::FRAME_COUNT,
            //     statistic: DiagnosticsOverlayStatistic::Smoothed,
            //     precision: 0,
            // },
        ],
    });
    // Basic 3D camera setup looking towards the origin
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(20.0, 20.0, 0.0).looking_at(Vec3::new(0.0, 10.0, 0.0), Vec3::Y),
        PanOrbitCamera::default(),
        PointCloudVisibilitySettings {
            min_radius: Some(30.0),
            point_budget: Some(1_000_000),
            // max_depth: Some(2),
            ..default()
        },
        Msaa::Off,
        RenderDebugOverlay {
            enabled: false,
            mode: RenderDebugMode::Depth,
            ..default()
        },
        PointCloudViewSettings {
            render_modes: RenderModes::all(),
            clipping_planes: vec![
                Vec4::new(0.0, -1.0, 0.0, -20.0),
                Vec4::new(0.0, 1.0, 0.0, 20.0),
            ],
            edl_enabled: false,
            ..default()
        },
        // NoIndirectDrawing,
    ));
}

fn setup_scene(
    mut meshes: ResMut<Assets<Mesh>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
    shared_state: Res<SharedState>,
) -> Result<()> {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(150.0, 150.0))),
        MeshMaterial3d(standard_materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.8,
            ..default()
        })),
        Transform::from_xyz(0.0, 1.8, 0.0),
        // Transform::from_xyz(0.0, -5.0, 0.0),
    ));

    let point_cloud_entity = commands.spawn_scene(point_cloud_scene(&shared_state)).id();
    commands
        .entity(shared_state.parent_entity)
        .add_child(point_cloud_entity);

    Ok(())
}

fn point_cloud_scene(shared_state: &SharedState) -> impl Scene {
    let point_cloud = shared_state.point_cloud.clone();
    let std_material = shared_state.std_material.clone();
    let simple_material = shared_state.simple_material.clone();
    let triangle = shared_state.triangle.clone();
    // let splat_settings = SplatSettings {
    //     point_size_mode: PointSizeMode::LocalSpace,
    //     radius: Some(0.5),
    //     orientation: SplatOrientation::Billboard,
    //     default_normal: Vec3::new(0.0, 0.0, 1.0),
    //     ..default()
    // };
    bsn! {
        Name("Scene Point Cloud")
        MyPointCloud
        PointCloud3d(point_cloud)
        // PointCloudMaterial3d<StandardPointCloudMaterial>(std_material)
        PointCloudMaterial3d<SimplePointCloudMaterial>(simple_material)
        SplatSettings {
            splat: OptionTemplate::Some(triangle),
            point_size_mode: PointSizeModeTemplate::LocalSpace,
            // point_size: 0.01,
            point_size: 1.0,
            adaptive_point_size: true,
            radius: OptionTemplate::Some(0.5),
            orientation: SplatOrientationTemplate::Billboard,
            default_normal: Vec3::new(0.0, 0.0, 1.0),
        }
    }
    // commands.spawn((
    //     MyPointCloud,
    //     ChildOf(shared_state.parent_entity),
    //     PointCloud3d(shared_state.point_cloud.clone()),
    //     PointCloudMaterial3d(shared_state.material.clone()),
    //     SplatSettings {
    //         point_size_mode: PointSizeMode::LocalSpace,
    //         radius: Some(0.5),
    //         orientation: SplatOrientation::Billboard,
    //         default_normal: Vec3::new(0.0, 0.0, 1.0),
    //         ..default()
    //     },
    // ));

    // Ok(())
}

fn setup_sun(mut commands: Commands) {
    // commands.spawn((
    //     DirectionalLight {
    //         color: Color::WHITE,
    //         illuminance: light_consts::lux::AMBIENT_DAYLIGHT, // ~10,000 lux
    //         shadow_maps_enabled: true,
    //         ..default()
    //     },
    //     Transform::from_rotation(Quat::from_rotation_x(-PI / 4.0)),
    //     PointCloudVisibilitySettings {
    //         min_radius: Some(30.0),
    //         point_budget: Some(1_000_000),
    //         ..default()
    //     },
    //     PointCloudViewSettings {
    //         render_modes: RenderModes::all(),
    //         clipping_planes: vec![
    //             Vec4::new(0.0, -1.0, 0.0, -20.0),
    //             Vec4::new(0.0, 1.0, 0.0, 20.0),
    //         ],
    //         edl_enabled: false,
    //         ..default()
    //     },
    // ));
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
        // info!("center point cloud {:?}", entity);
        // let center = aabb.center;
        // commands
        //     .entity(entity)
        //     .insert(Transform::from_translation(Vec3 {
        //         x: -center.x,
        //         y: -center.y,
        //         z: 0.0, // Keep the original altitude
        //     }));

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
    cameras: Query<
        (
            Option<&mut PanOrbitCamera>,
            &mut PointCloudVisibilitySettings,
        ),
        Without<DirectionalLight>,
    >,
    view_settings: Query<&mut PointCloudViewSettings>,
    lights: Query<&mut PointCloudVisibilitySettings, With<DirectionalLight>>,
    ui_state: Res<UiState>,
    ui_settings: Res<UiSettings>,
    mut simple_materials: ResMut<Assets<SimplePointCloudMaterial>>,
    shared_state: Res<SharedState>,
    mut xray_enabled: Local<bool>,
    mut weighted_splats: Local<bool>,
) {
    for (pan_orbit_camera, mut visibility_settings) in cameras {
        if let Some(mut pan_orbit_camera) = pan_orbit_camera {
            pan_orbit_camera.enabled = !ui_state.dragging && !ui_state.hovering;
        }
        visibility_settings.point_budget = Some(ui_settings.point_budget);
        visibility_settings.min_radius = Some(ui_settings.min_node_size);
    }

    for mut view_settings in view_settings {
        view_settings.clipping_planes[0].w = ui_settings.plane_distance_0;
        view_settings.clipping_planes[1].w = ui_settings.plane_distance_1;
        view_settings.edl_enabled = ui_settings.edl_enabled;
    }
    for mut visibility_settings in lights {
        visibility_settings.point_budget = Some(ui_settings.light_point_budget);
        visibility_settings.min_radius = Some(ui_settings.light_min_node_size);
    }

    if let Some(mut simple_material) = simple_materials.get_mut(shared_state.simple_material.id()) {
        if *xray_enabled != ui_settings.xray {
            *xray_enabled = ui_settings.xray;
            simple_material.render_mode = match ui_settings.xray {
                true => RenderMode::XRay,
                false => RenderMode::Normal,
            };
        }
        if *weighted_splats != ui_settings.weighted_splats {
            *weighted_splats = ui_settings.weighted_splats;
            simple_material.weighted_splats = ui_settings.weighted_splats;
        }
    }
}

fn despawn_observer(
    _event: On<UiClickDespawnEvent>,
    query: Query<Entity, With<MyPointCloud>>,
    mut commands: Commands,
) {
    for entity in query {
        commands.entity(entity).despawn_children().despawn();
    }
}

fn spawn_observer(
    _event: On<UiClickSpawnEvent>,
    shared_state: Res<SharedState>,
    mut commands: Commands,
) {
    // let entity = commands.spawn_scene(point_cloud_scene(&shared_state)).id();
    // commands
    //     .entity(shared_state.parent_entity)
    //     .add_child(entity);
    commands
        .entity(shared_state.parent_entity)
        .queue_spawn_related_scenes::<Children>(bsn_list![point_cloud_scene(&shared_state)]);
}

fn set_splat_orientation_observer(
    event: On<UiClickSetSplatOrientation>,
    mut query: Query<&mut SplatSettings, With<MyPointCloud>>,
) {
    for mut splat_settings in &mut query {
        splat_settings.orientation = *event.event().deref();
    }
}
