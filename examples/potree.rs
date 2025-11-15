#[path = "helpers/camera_controller.rs"]
mod camera_controller;

use crate::camera_controller::{CameraController, CameraControllerPlugin};
use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig};
use bevy::tasks::Task;
use bevy::text::FontSmoothing;
#[cfg(all(not(feature = "webgl"), not(feature = "webgpu")))]
use bevy::window::PresentMode;
use bevy::{prelude::*, render::view::NoIndirectDrawing};
use bevy_asset::UnapprovedPathMode;
use bevy_color::palettes::basic::GREEN;
use bevy_ecs::world::CommandQueue;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_panorbit_camera::PanOrbitCameraPlugin;
use bevy_pointcloud::PointCloudPlugin;
use bevy_pointcloud::point_cloud_material::{PointCloudMaterial, PointCloudMaterial3d};
use bevy_pointcloud::potree::prelude::*;
use bevy_pointcloud::render::PointCloudRenderMode;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(AssetPlugin {
            unapproved_path_mode: UnapprovedPathMode::Allow,
            ..Default::default()
        }),
        EguiPlugin::default(),
        WorldInspectorPlugin::new(),
        PanOrbitCameraPlugin,
        PointCloudPlugin,
        CameraControllerPlugin,
    ));

    #[cfg(all(not(feature = "webgl"), not(feature = "webgpu")))]
    app.add_plugins(FpsOverlayPlugin {
        config: FpsOverlayConfig {
            text_config: TextFont {
                // Here we define size of our overlay
                font_size: 42.0,
                // If we want, we can use a custom font
                font: default(),
                // We could also disable font smoothing,
                font_smoothing: FontSmoothing::default(),
                ..default()
            },
            // We can also change color of the overlay
            text_color: Color::Srgba(GREEN),
            // We can also set the refresh interval for the FPS counter
            refresh_interval: core::time::Duration::from_millis(100),
            enabled: true,
            frame_time_graph_config: {
                #[cfg(any(feature = "webgl", feature = "webgpu"))]
                {
                    FrameTimeGraphConfig {
                        enabled: false,
                        ..Default::default()
                    }
                }
                #[cfg(all(not(feature = "webgl"), not(feature = "webgpu")))]
                {
                    FrameTimeGraphConfig::default()
                }
            },
            ..Default::default()
        },
    });
    app.add_systems(Startup, (setup_window, setup, load_pointcloud, load_meshes))
        .run();
}

fn setup_window(mut windows: Query<&mut Window>) {
    let mut window = windows.single_mut().unwrap();

    #[cfg(all(not(feature = "webgl"), not(feature = "webgpu")))]
    {
        window.present_mode = PresentMode::Mailbox;
    }
}

fn setup(mut commands: Commands) {
    // camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(5.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        // We need this component because we use `draw_indexed` and `draw`
        // instead of `draw_indirect_indexed` and `draw_indirect` in
        // `DrawMeshInstanced::render`.
        NoIndirectDrawing,
        CameraController::default(),
        // PanOrbitCamera::default(),
        // disable msaa for WASM/WebGL (but works in native mode)
        Msaa::Off,
        PointCloudRenderMode {
            use_edl: true,
            edl_radius: 1.4,
            edl_strength: 0.4,
            edl_neighbour_count: 4,
            ..Default::default()
        },
        // Use this camera for potree octree loading
        PotreeMainCamera,
    ));
}

fn load_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // let sphere = meshes.add(Sphere::default().mesh().ico(5).unwrap());
    // commands.spawn((
    //     Mesh3d(sphere),
    //     MeshMaterial3d(materials.add(Color::from(RED))),
    //     Transform::from_translation(Vec3::new(0.0, 2.0, 1.0)),
    //     NotShadowCaster,
    // ));

    // commands.spawn((
    //     PointLight {
    //         shadows_enabled: true,
    //         intensity: 10_000_000.,
    //         range: 100.0,
    //         shadow_depth_bias: 0.2,
    //         ..default()
    //     },
    //     Transform::from_xyz(8.0, 16.0, 8.0),
    // ));
    //
    // commands.spawn((
    //     Mesh3d(meshes.add(Plane3d::default().mesh().size(50.0, 50.0).subdivisions(10))),
    //     MeshMaterial3d(materials.add(Color::from(SILVER))),
    // ));
}

#[derive(Component)]
pub struct MyMaterial(Handle<PointCloudMaterial>);

fn load_pointcloud(
    mut commands: Commands,
    mut point_cloud_materials: ResMut<Assets<PointCloudMaterial>>,
    // mut point_clouds: ResMut<Assets<PointCloud>>,
    asset_server: Res<AssetServer>,
    // mut gizmo_assets: ResMut<Assets<GizmoAsset>>,
) {
    let my_material = point_cloud_materials.add(PointCloudMaterial {
        point_size: 30.0,
        min_point_size: 2.0,
        max_point_size: 50.0,
        ..default()
    });
    commands.spawn(MyMaterial(my_material.clone()));

    let potree_point_cloud_handle: Handle<PotreePointCloud> =
        asset_server.load("potree/heidentor/metadata.json");
    // asset_server.load("/home/romain/Documents/Potree/Liban");

    commands.spawn((
        PotreePointCloud3d {
            handle: potree_point_cloud_handle.clone(),
        },
        DrawPotreeGizmo,
        Transform::from_rotation(Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2)),
        PointCloudMaterial3d(my_material.clone()),
    ));

    // for i in 0..8 {
    //     for j in 0..8 {
    //         commands.spawn((
    //             PotreePointCloud3d {
    //                 handle: potree_point_cloud_handle.clone(),
    //             },
    //             // DrawPotreeGizmo,
    //             Transform::from_rotation(Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2))
    //                 .with_translation(Vec3::new(10.0 * i as f32, 0.0, 10.0 * j as f32)),
    //             PointCloudMaterial3d(my_material.clone()),
    //         ));
    //     }
    // }

    // commands.spawn((
    //     PotreePointCloud3d {
    //         handle: potree_point_cloud_handle,
    //     },
    //     DrawPotreeGizmo,
    //     Transform::from_rotation(Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2))
    //         .with_translation(Vec3::new(10.0, 0.0, 0.0)),
    //     PointCloudMaterial3d(my_material.clone()),
    // ));

    return;

    // let point_cloud = asset_server.load::<PointCloud>("potree/heidentor/metadata.json");
    // commands.spawn((
    //     PointCloud3d(point_cloud),
    //     PointCloudMaterial3d(my_material.clone()),
    //     MainPointCloud,
    // ));
}
