#[path = "helpers/camera_controller.rs"]
mod camera_controller;

use bevy::color::palettes::basic::GREEN;
use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin};
use bevy::text::FontSmoothing;
use bevy::window::PresentMode;
use bevy::{prelude::*, render::view::NoIndirectDrawing};
use bevy_pointcloud::PointCloudPlugin;
use bevy_pointcloud::pointcloud::{PointCloud, PointCloudData};
use bevy_pointcloud::render::PointCloud3d;
use camera_controller::{CameraController, CameraControllerPlugin};
use rand::Rng;

/// This example uses a shader source file from the assets subdirectory

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, CameraControllerPlugin, PointCloudPlugin))
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
        .add_systems(Startup, (setup_window, setup, load_pointcloud))
        .run();
}

fn setup_window(mut windows: Query<&mut Window>) {
    let mut window = windows.single_mut().unwrap();
    window.present_mode = PresentMode::Immediate;
}

fn setup(mut commands: Commands) {
    // camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
        // We need this component because we use `draw_indexed` and `draw`
        // instead of `draw_indirect_indexed` and `draw_indirect` in
        // `DrawMeshInstanced::render`.
        NoIndirectDrawing,
        CameraController::default(),
    ));
}

fn load_pointcloud(mut commands: Commands, mut point_clouds: ResMut<Assets<PointCloud>>) {
    // Generate a random point cloud
    let mut rng = rand::rng();
    let points = (0..1000000)
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

    // let mut points = Vec::new();
    //
    // use las::Reader;
    // let mut reader = Reader::from_path("assets/pointclouds/Palac_Moszna.laz").unwrap();
    // // let mut reader = Reader::from_path("assets/pointclouds/G_Sw_Anny.laz").unwrap();
    //
    //
    // let mut min = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
    // let mut max = Vec3::new(f32::MIN, f32::MIN, f32::MIN);
    //
    // reader.points().into_iter().for_each(|point| {
    //     let point = point.unwrap();
    //     let vec = Vec3::new(point.x as f32, point.y as f32, point.z as f32);
    //
    //     min = min.min(vec);
    //     max = max.max(vec);
    // });
    //
    // reader.seek(0).unwrap();
    //
    // for wrapped_point in reader.points() {
    //     let point = wrapped_point.unwrap();
    //
    //     if let Some(color) = point.color {
    //         points.push(PointCloudData {
    //             position: Vec3::new(
    //                 point.x as f32 - min.x,
    //                 point.z as f32 - min.z,
    //                 point.y as f32 - min.y,
    //             ),
    //             point_size: 500.0,
    //             // color,
    //             color: [
    //                 color.red as f32 / u16::MAX as f32,
    //                 color.green as f32 / u16::MAX as f32,
    //                 color.blue as f32 / u16::MAX as f32,
    //                 1.0,
    //             ],
    //         });
    //     }
    // }

    // Create chunks of point cloud
    // TODO chunk it using octrees or BVH
    let step = points.len() / 64;

    for i in 0..4 {
        for j in 0..4 {
            for k in 0..4 {
                let start = (i + j + k) * 64;
                let end = points.len().min(start + step);
                let point_cloud = PointCloud {
                    points: (&points[start..end]).to_vec(),
                };
                info!("Spawn a mesh with {} points", point_cloud.points.len());
                commands.spawn((
                    PointCloud3d(point_clouds.add(point_cloud)),
                    Transform::from_xyz(i as f32 * 30.0, j as f32 * 30.0, k as f32 * 30.0),
                    // Transform::from_xyz(0.0, 0.0, 0.0),
                ));
            }
        }
    }
}
