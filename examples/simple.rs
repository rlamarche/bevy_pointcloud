#[path = "helpers/camera_controller.rs"]
mod camera_controller;

use bevy::color::palettes::basic::GREEN;
use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin};
use bevy::text::FontSmoothing;
use bevy::{
    prelude::*,
    render::{
        render_resource::*,
        view::{NoFrustumCulling, NoIndirectDrawing},
    },
};
use bevy_asset::RenderAssetUsages;
use bevy_pointcloud::PointCloudPlugin;
use bevy_pointcloud::point_cloud::{PointCloudData, PointCloudInstance};
use bevy_render::mesh::Indices;
use camera_controller::{CameraController, CameraControllerPlugin};
use rand::Rng;
use std::sync::Arc;

/// This example uses a shader source file from the assets subdirectory
const SHADER_ASSET_PATH: &str = "shaders/instancing.wgsl";

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
    let window = windows.single_mut().unwrap();
    // window.present_mode = PresentMode::Immediate;
}

fn setup(mut commands: Commands, meshes: ResMut<Assets<Mesh>>) {
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

fn load_pointcloud(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [-0.5, -0.5, 0.0],
            [0.5, -0.5, 0.0],
            [0.5, 0.5, 0.0],
            [-0.5, 0.5, 0.0],
        ],
    )
    .with_inserted_indices(Indices::U32(vec![0, 1, 2, 2, 3, 0]));

    let mut rng = rand::rng();
    let instance_data = (0..100000)
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
                point_size: 50.0,
                color,
            }
        })
        .collect::<Vec<_>>();
    //
    // let mut instance_data = Vec::new();
    //
    // use las::Reader;
    // let mut reader = Reader::from_path("assets/pointclouds/Palac_Moszna.laz").unwrap();
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
    //         instance_data.push(InstanceData {
    //             position: Vec3::new(
    //                 point.x as f32 - min.x,
    //                 point.z as f32 - min.z,
    //                 point.y as f32 - min.y,
    //             ),
    //             scale: 1.0,
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

    info!("Points count: {}", instance_data.len());

    // let mesh_handle = meshes.add(mesh);

    let step = 10000;
    for i in (0..instance_data.len()).step_by(step) {
        let max = instance_data.len().min(i + step);
        let instance_data = (&instance_data[i..max]).to_vec();

        info!("Spawn a mesh with {} points", instance_data.len());
        commands.spawn((
            // Mesh3d(mesh_handle.clone()),
            Mesh3d(meshes.add(mesh.clone())),
            PointCloudInstance(Arc::new(instance_data)),
            NoFrustumCulling,
        ));
    }
}
