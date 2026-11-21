#![recursion_limit = "256"]
#![expect(missing_docs, reason = "Not all docs are written yet.")]
mod ui;

use std::borrow::Cow;

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
    pbr::diagnostic::MaterialAllocatorDiagnosticPlugin,
    prelude::*,
    render::diagnostic::MeshAllocatorDiagnosticPlugin,
    window::PresentMode,
};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_pointcloud::{
    las::LasLoaderSettings, prelude::*, render::SplatMeshes, CopcLoader, MinimalPointCloudMaterial,
    PointCloudVisiblityPlugin, RenderMode, RenderModes, StandardPointCloudMaterial,
};

// --- RESOURCES AND STRUCTURES ---

#[derive(Resource, Debug)]
struct LaunchConfig {
    point_cloud_path: String,
}

#[derive(Resource)]
struct SharedState {
    // pub std_material: Handle<StandardPointCloudMaterial>,
    // pub simple_material: Handle<SimplePointCloudMaterial>,
    pub minimal_material: Handle<MinimalPointCloudMaterial>,
    pub root_entity: Entity,
}

#[derive(Component, FromTemplate)]
struct MyPointCloud;

// --- MAIN FUNCTION ---

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let point_cloud_path = args.first().cloned().expect("missing point cloud path");
    let config = LaunchConfig { point_cloud_path };

    let mut app = App::new();

    app
        // .insert_resource(DefaultOpaqueRendererMethod::deferred())
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
            ..default()
        }))
        .add_plugins((
            FrameTimeDiagnosticsPlugin::default(),
            MeshAllocatorDiagnosticPlugin,
            EntityCountDiagnosticsPlugin::default(),
            MaterialAllocatorDiagnosticPlugin::<StandardMaterial>::default(),
            SystemInformationDiagnosticsPlugin,
            DiagnosticsOverlayPlugin,
        ))
        .add_plugins(PanOrbitCameraPlugin)
        .add_plugins(PointCloudPlugin::default())
        // Startup Systems
        .add_systems(
            Startup,
            (
                setup_window,
                init_shared_state,
                setup_camera,
                setup_scene.after(init_shared_state),
            ),
        )
        // PostUpdate Systems
        .add_systems(PostUpdate, center_point_cloud)
        .insert_resource(config);

    app.run();
}

fn setup_window(mut windows: Query<&mut Window>) {
    let mut window = windows.single_mut().unwrap();
    window.present_mode = PresentMode::Mailbox;
}

fn init_shared_state(
    // mut pc_standard_materials: ResMut<Assets<StandardPointCloudMaterial>>,
    // mut pc_simple_materials: ResMut<Assets<SimplePointCloudMaterial>>,
    mut pc_minimal_materials: ResMut<Assets<MinimalPointCloudMaterial>>,
    mut commands: Commands,
    splat_meshes: Res<SplatMeshes>,
) -> Result<()> {
    //     let std_material = pc_standard_materials.add(StandardMaterial {
    //         cull_mode: None,
    //         opaque_render_method: bevy::material::OpaqueRendererMethod::Forward,
    //         // alpha_mode: AlphaMode::Blend,
    //         ..default()
    //     });
    //
    //     let simple_material = pc_simple_materials.add(SimplePointCloudMaterial {
    //         cull_mode: None,
    //         render_mode: RenderMode::Normal,
    //         weighted_splats: true,
    //         // opaque_render_method: bevy::material::OpaqueRendererMethod::Deferred,
    //         // alpha_mode: AlphaMode::Blend,
    //         ..default()
    //     });

    let minimal_material = pc_minimal_materials.add(MinimalPointCloudMaterial {
        // cull_mode: None,
        render_mode: RenderMode::Normal,
        weighted_splats: true,
        // opaque_render_method: bevy::material::OpaqueRendererMethod::Deferred,
        // alpha_mode: AlphaMode::Blend,
        ..default()
    });

    let root_entity = commands
        .spawn((
            Name::new("Root entity"),
            Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            Visibility::default(),
        ))
        .id();

    commands.insert_resource(SharedState {
        // std_material,
        // simple_material,
        minimal_material,
        root_entity,
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
            PointCloudVisiblityPlugin::VISIBILITY_CHECK_TIME.into(),
            PointCloudVisiblityPlugin::BUDGET.into(),
        ],
    });
    // Basic 3D camera setup looking towards the origin
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(20.0, 20.0, 0.0).looking_at(Vec3::new(0.0, 10.0, 0.0), Vec3::Y),
        PanOrbitCamera::default(),
        PointCloudVisibilitySettings {
            min_radius: Some(30.0),
            point_budget: Some(10_000_000),
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
            edl_enabled: true,
            ..default()
        },
    ));
}

fn setup_scene(
    launch_config: Res<LaunchConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut point_clouds: ResMut<Assets<PointCloud>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
    shared_state: Res<SharedState>,
    asset_server: Res<AssetServer>,
    point_cloud_server: Res<PointCloudServer>,
    splat_meshes: Res<SplatMeshes>,
) -> Result<()> {
    // commands.spawn((
    //     Mesh3d(meshes.add(Plane3d::default().mesh().size(1000.0, 1000.0))),
    //     MeshMaterial3d(standard_materials.add(StandardMaterial {
    //         base_color: Color::WHITE,
    //         perceptual_roughness: 0.8,
    //         ..default()
    //     })),
    //     Transform::from_xyz(0.0, 0.0, 0.0),
    //     // Transform::from_xyz(0.0, -5.0, 0.0),
    // ));

    let point_cloud = if launch_config
        .point_cloud_path
        .to_lowercase()
        .contains("copc")
    {
        point_cloud_server.load_with_settings::<CopcLoader<FileSource>>(
            FileSource::open(&launch_config.point_cloud_path).unwrap(),
            |settings| {
                settings.compute_offset = true;
                settings.min_color = 0.0;
                settings.max_color = 255.0;
            },
        )
    } else {
        asset_server
            .load_builder()
            .with_settings::<LasLoaderSettings>(|settings| {
                settings.min_color = 0.0;
                settings.max_color = 255.0;
            })
            .load(&launch_config.point_cloud_path)
    };

    commands.spawn((
        MyPointCloud,
        ChildOf(shared_state.root_entity),
        PointCloud3d(point_cloud.clone()),
        PointCloudMaterial3d(shared_state.minimal_material.clone()),
        SplatSettings {
            splat: Some(splat_meshes.triangle_mesh.clone()),
            point_size_mode: PointSizeMode::LocalSpace,
            point_size: 1.0,
            radius: Some(0.5),
            orientation: SplatOrientation::Billboard,
            default_normal: Vec3::new(0.0, 0.0, 1.0),
            ..default()
        },
    ));

    Ok(())
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
        info!(
            "center point cloud {:?} for aabb {:#?}",
            entity, aabb.center
        );
        let center = aabb.center;
        commands
            .entity(entity)
            .insert(Transform::from_translation(Vec3 {
                x: -center.x,
                y: -center.y,
                z: -center.z / 2.0,
            }));

        last_aabb = Some(aabb);
    }

    //     let Some(aabb) = last_aabb else {
    //         return;
    //     };
    //
    //     let (camera_transform, mut pan_orbit_camera) = camera.single_mut().unwrap();
    //
    //     let target_focus = Vec3::new(0.0, aabb.half_extents.y, 0.0);
    //     let (yaw, pitch, radius) = calculate_from_translation_and_focus(
    //         camera_transform.translation,
    //         target_focus,
    //         pan_orbit_camera.axis,
    //     );
    //
    //     pan_orbit_camera.target_yaw = yaw;
    //     pan_orbit_camera.target_pitch = pitch;
    //     pan_orbit_camera.target_radius = radius;
    //     pan_orbit_camera.target_focus = target_focus;
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
