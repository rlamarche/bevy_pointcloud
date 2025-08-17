#[path = "helpers/camera_controller.rs"]
mod camera_controller;

use bevy::color::palettes::basic::GREEN;
use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin};
use bevy::text::FontSmoothing;
use bevy::window::PresentMode;
use bevy::{prelude::*, render::view::NoIndirectDrawing};
use bevy_color::palettes::basic::{RED, SILVER};
use bevy_pointcloud::PointCloudPlugin;
use bevy_pointcloud::loader::las::LasLoaderPlugin;
use bevy_pointcloud::point_cloud::{PointCloud, PointCloud3d, PointCloudData};
use bevy_pointcloud::point_cloud_material::{PointCloudMaterial, PointCloudMaterial3d};
use bevy_pointcloud::render::PointCloudRenderMode;
use camera_controller::{CameraController, CameraControllerPlugin};
use rand::Rng;

/// This example uses a shader source file from the assets subdirectory

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            CameraControllerPlugin,
            PointCloudPlugin,
            LasLoaderPlugin,
        ))
        .add_plugins(FpsOverlayPlugin {
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
            },
        })
        .add_systems(Startup, (setup_window, setup, load_pointcloud, load_meshes))
        .add_systems(Update, update_material_on_keypress)
        .run();
}

fn setup_window(mut windows: Query<&mut Window>) {
    let mut window = windows.single_mut().unwrap();
    // window.present_mode = PresentMode::Immediate;
}

fn setup(mut commands: Commands) {
    // camera
    commands.spawn((
        Camera3d::default(),
        Projection::from(PerspectiveProjection {
            fov: core::f32::consts::PI / 4.0,
            near: 0.1,
            far: 1000.0,
            aspect_ratio: 1.0,
        }),
        Transform::from_xyz(0.0, 0.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
        // We need this component because we use `draw_indexed` and `draw`
        // instead of `draw_indirect_indexed` and `draw_indirect` in
        // `DrawMeshInstanced::render`.
        NoIndirectDrawing,
        CameraController::default(),
        // disable msaa for WASM/WebGL (but works in native mode)
        Msaa::Off,
        PointCloudRenderMode {
            use_edl: true,
            edl_radius: 2.8,
            edl_strength: 1.0,
            edl_neighbour_count: 8,
            ..Default::default()
        },
    ));
}

fn load_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let sphere = meshes.add(Sphere::default().mesh().ico(5).unwrap());
    commands.spawn((
        Mesh3d(sphere),
        MeshMaterial3d(materials.add(Color::from(RED))),
    ));

    commands.spawn((
        PointLight {
            shadows_enabled: true,
            intensity: 10_000_000.,
            range: 100.0,
            shadow_depth_bias: 0.2,
            ..default()
        },
        Transform::from_xyz(8.0, 16.0, 8.0),
    ));

    // ground plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(50.0, 50.0).subdivisions(10))),
        MeshMaterial3d(materials.add(Color::from(SILVER))),
    ));
}

#[derive(Component)]
pub struct MyMaterial(Handle<PointCloudMaterial>);

fn load_pointcloud(
    mut commands: Commands,
    mut point_clouds: ResMut<Assets<PointCloud>>,
    mut point_cloud_materials: ResMut<Assets<PointCloudMaterial>>,
    asset_server: Res<AssetServer>,
) {
    let my_material = point_cloud_materials.add(PointCloudMaterial { point_size: 30.0 });
    commands.spawn(MyMaterial(my_material.clone()));

    let point_cloud = asset_server.load::<PointCloud>("pointclouds/lion_takanawa.copc.laz");
    commands.spawn((
        PointCloud3d(point_cloud),
        PointCloudMaterial3d(my_material.clone()),
    ));

    // Generate a random point cloud
    let mut rng = rand::rng();
    let nb_points = 1000000;

    let points = (0..nb_points)
        .map(|_| {
            let position = Vec3::new(
                rng.random_range(-10.0..10.0),
                rng.random_range(-10.0..10.0),
                rng.random_range(-10.0..10.0),
            );
            let color = [
                rng.random_range(0.0..1.0),
                rng.random_range(0.0..1.0),
                rng.random_range(0.0..1.0),
                1.0,
            ];
            PointCloudData {
                position,
                point_size: rng.random_range(100.0..300.0),
                color,
            }
        })
        .collect::<Vec<_>>();

    // Create chunks of point cloud
    // TODO chunk it using octrees or BVH
    let step = points.len() / 64;

    for i in 0..4 {
        for j in 0..4 {
            for k in 0..4 {
                let block_index = i + j * 4 + k * 16;
                let start = block_index * step;
                let end = ((block_index + 1) * step).min(points.len());
                let point_cloud = PointCloud {
                    points: (&points[start..end]).to_vec(),
                };
                // info!("Spawn a mesh with {} points", point_cloud.points.len());
                commands.spawn((
                    PointCloud3d(point_clouds.add(point_cloud)),
                    PointCloudMaterial3d(my_material.clone()),
                    Transform::from_xyz(i as f32 * 30.0, j as f32 * 30.0, k as f32 * 30.0),
                ));
            }
        }
    }
}

fn update_material_on_keypress(
    time: Res<Time>,
    key_input: Res<ButtonInput<KeyCode>>,
    my_material: Query<&MyMaterial>,
    mut point_cloud_materials: ResMut<Assets<PointCloudMaterial>>,
) {
    let dt = time.delta_secs();

    let my_material = my_material.single().unwrap();
    let point_cloud_material = point_cloud_materials.get_mut(&my_material.0).unwrap();

    if key_input.pressed(KeyCode::NumpadAdd) {
        point_cloud_material.point_size += 1.0;
    }
    if key_input.pressed(KeyCode::NumpadSubtract) {
        point_cloud_material.point_size -= 1.0;
    }
}
