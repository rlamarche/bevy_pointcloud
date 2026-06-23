#[path = "helpers/camera_controller.rs"]
mod camera_controller;

use std::{ops::Neg, sync::Arc};

use bevy::DefaultPlugins;
use bevy_app::prelude::*;
use bevy_asset::{AssetPlugin, AssetServer, Assets, Handle};
use bevy_camera::{primitives::Aabb, Camera3d, PerspectiveProjection, Projection};
#[cfg(all(not(feature = "webgl"), not(feature = "webgpu")))]
use bevy_color::palettes::basic::{GREEN, RED};
use bevy_color::{palettes::basic::SILVER, Color};
#[cfg(all(not(feature = "webgl"), not(feature = "webgpu")))]
use bevy_dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin};
use bevy_ecs::prelude::*;
use bevy_input::{prelude::*, ButtonInput};
use bevy_light::{NotShadowCaster, PointLight};
use bevy_math::prelude::*;
use bevy_mesh::{Mesh, Mesh3d, Meshable};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_pbr::{MeshMaterial3d, StandardMaterial};
use bevy_pointcloud::{
    loader::las::LasLoaderPlugin,
    point::RGBPoint,
    point_cloud::{
        PointCloud, PointCloud3d, PointCloudGpuMapper, PointCloudGpuMapperPlugin,
        PointCloudIdentityGpuMapper, PointCloudsPlugin,
    },
    point_cloud_material::{
        PointCloudMaterial3d, SimplePointCloudMaterial, SimplePointCloudMaterialPlugin,
    },
    render::PointCloudRenderMode,
};
use bevy_reflect::TypePath;
use bevy_render::{prelude::*, view::NoIndirectDrawing};
use bevy_text::{FontSmoothing, TextFont};
use bevy_transform::prelude::*;
use bevy_utils::default;
use bevy_window::{PresentMode, Window};
use rand::Rng;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                watch_for_changes_override: Some(true),
                ..Default::default()
            }),
            PanOrbitCameraPlugin,
            PointCloudsPlugin::<RGBPoint>::default(),
            PointCloudGpuMapperPlugin::<PointCloudIdentityGpuMapper<RGBPoint>>::default(),
            PointCloudGpuMapperPlugin::<MyPointCloudGpuMapper>::default(),
            SimplePointCloudMaterialPlugin::<PointCloudIdentityGpuMapper<RGBPoint>>::default(),
            SimplePointCloudMaterialPlugin::<MyPointCloudGpuMapper>::default(),
            LasLoaderPlugin::<RGBPoint>::default(),
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
                ..default()
            },
        })
        .add_systems(Startup, (setup_window, setup, load_pointcloud, load_meshes))
        .add_systems(PreUpdate, update_material_on_keypress)
        .add_systems(PreUpdate, center_point_cloud)
        .run();
}

fn setup_window(mut windows: Query<&mut Window>) {
    #[allow(unused, unused_mut)]
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
        Projection::from(PerspectiveProjection {
            fov: core::f32::consts::PI / 4.0,
            near: 0.1,
            far: 1000.0,
            aspect_ratio: 1.0,
            ..Default::default()
        }),
        Transform::from_xyz(5.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        // We need this component because we use `draw_indexed` and `draw`
        // instead of `draw_indirect_indexed` and `draw_indirect` in
        // `DrawMeshInstanced::render`.
        NoIndirectDrawing,
        // CameraController::default(),
        PanOrbitCamera::default(),
        // disable msaa for WASM/WebGL (but works in native mode)
        Msaa::Off,
        PointCloudRenderMode {
            use_edl: true,
            edl_radius: 2.8,
            edl_strength: 0.4,
            edl_neighbour_count: 4,
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
        Transform::from_translation(Vec3::new(0.0, 2.0, 1.0)),
        NotShadowCaster,
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

    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(50.0, 50.0).subdivisions(10))),
        MeshMaterial3d(materials.add(Color::from(SILVER))),
    ));
}

#[derive(Component)]
pub struct MyMaterial(Handle<SimplePointCloudMaterial>);

#[derive(Component)]
struct MainPointCloud(Vec3);

#[derive(Component, TypePath)]
struct MyPointCloudGpuMapper;

impl PointCloudGpuMapper for MyPointCloudGpuMapper {
    type Point = RGBPoint;

    type GpuPoint = RGBPoint;

    type Param = ();

    fn convert(
        points: std::sync::Arc<Vec<Self::Point>>,
        _param: &mut bevy_ecs::system::SystemParamItem<Self::Param>,
    ) -> Result<
        std::sync::Arc<Vec<Self::GpuPoint>>,
        bevy_pointcloud::render_asset::PrepareAssetComponentError<PointCloud<Self::Point>>,
    > {
        let mut rng = rand::rng();

        let points: Vec<_> = points
            .iter()
            .map(|p| {
                RGBPoint::new(
                    p.position,
                    Vec4::new(
                        rng.random_range(0.0..1.0),
                        rng.random_range(0.0..1.0),
                        rng.random_range(0.0..1.0),
                        1.0,
                    ),
                )
            })
            .collect();

        Ok(Arc::new(points))
    }
}

fn load_pointcloud(
    mut commands: Commands,
    mut point_cloud_materials: ResMut<Assets<SimplePointCloudMaterial>>,
    _point_clouds: ResMut<Assets<PointCloud<RGBPoint>>>,
    asset_server: Res<AssetServer>,
) {
    let my_material = point_cloud_materials.add(SimplePointCloudMaterial {
        point_size: 30.0,
        ..default()
    });
    let my_second_material = point_cloud_materials.add(SimplePointCloudMaterial {
        point_size: 15.0,
        ..default()
    });
    commands.spawn(MyMaterial(my_material.clone()));
    commands.spawn(MyMaterial(my_second_material.clone()));

    let point_cloud =
        asset_server.load::<PointCloud<RGBPoint>>("pointclouds/lion_takanawa.copc.laz");
    commands.spawn((
        PointCloud3d(point_cloud.clone()),
        PointCloudMaterial3d::<SimplePointCloudMaterial, PointCloudIdentityGpuMapper<RGBPoint>>::from(my_material),
        MainPointCloud(Vec3::ZERO),
        PointCloudIdentityGpuMapper::<RGBPoint>::default(),
        // MyPointCloudGpuMapper,
    ));
    commands.spawn((
        PointCloud3d(point_cloud),
        PointCloudMaterial3d::<SimplePointCloudMaterial, MyPointCloudGpuMapper>::from(
            my_second_material,
        ),
        MainPointCloud(Vec3::new(5.0, 0.0, 0.0)),
        MyPointCloudGpuMapper,
    ));

    // Generate a random point cloud
    // let mut rng = rand::rng();
    // let nb_points = 1000000;
    //
    // let points = (0..nb_points)
    //     .map(|_| {
    //         let position = Vec3::new(
    //             rng.random_range(-10.0..10.0),
    //             rng.random_range(-10.0..10.0),
    //             rng.random_range(-10.0..10.0),
    //         );
    //         let color = [
    //             rng.random_range(0.0..1.0),
    //             rng.random_range(0.0..1.0),
    //             rng.random_range(0.0..1.0),
    //             1.0,
    //         ];
    //         PointCloudData {
    //             position,
    //             point_size: rng.random_range(100.0..300.0),
    //             color,
    //         }
    //     })
    //     .collect::<Vec<_>>();

    // Create chunks of point cloud
    // TODO chunk it using octrees or BVH
    // let my_second_material = point_cloud_materials.add(PointCloudMaterial {
    //     point_size: -1.0,
    //     ..default()
    // });
    //
    // let step = points.len() / 64;
    // for i in 0..4 {
    //     for j in 0..4 {
    //         for k in 0..4 {
    //             let block_index = i + j * 4 + k * 16;
    //             let start = block_index * step;
    //             let end = ((block_index + 1) * step).min(points.len());
    //             let point_cloud = PointCloud {
    //                 points: (&points[start..end]).to_vec(),
    //             };
    //             // info!("Spawn a mesh with {} points", point_cloud.points.len());
    //             commands.spawn((
    //                 PointCloud3d(point_clouds.add(point_cloud)),
    //                 PointCloudMaterial3d(my_second_material.clone()),
    //                 Transform::from_xyz(i as f32 * 30.0, j as f32 * 30.0, k as f32 * 30.0),
    //             ));
    //         }
    //     }
    // }
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

#[allow(clippy::type_complexity)]
fn center_point_cloud(
    mut camera: Query<
        (&mut Transform, &mut PanOrbitCamera),
        (With<Camera3d>, Without<PointCloud3d<RGBPoint>>),
    >,
    mut query: Query<
        (&Aabb, &mut Transform, &MainPointCloud),
        (With<PointCloud3d<RGBPoint>>, Changed<Aabb>),
    >,
) {
    for (aabb, mut transform, main_pointcloud) in query.iter_mut() {
        // Center point cloud
        *transform = Transform::from_translation(
            (aabb.center.neg() + Vec3A::new(0.0, aabb.half_extents.y, 0.0)).to_vec3()
                + main_pointcloud.0,
        );

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
}

fn update_material_on_keypress(
    key_input: Res<ButtonInput<KeyCode>>,
    my_material: Query<&MyMaterial>,
    mut point_cloud_materials: ResMut<Assets<SimplePointCloudMaterial>>,
    mut point_cloud_render_mode: Query<&mut PointCloudRenderMode>,
) {
    if key_input.pressed(KeyCode::NumpadAdd) || key_input.pressed(KeyCode::NumpadSubtract) {
        for my_material in my_material.iter() {
            let point_cloud_material = point_cloud_materials.get_mut(&my_material.0).unwrap();

            if key_input.pressed(KeyCode::NumpadAdd) {
                point_cloud_material.point_size += 1.0;
            }
            if key_input.pressed(KeyCode::NumpadSubtract) {
                point_cloud_material.point_size -= 1.0;
            }
        }
    }

    let mut point_cloud_render_mode = point_cloud_render_mode.single_mut().unwrap();

    if key_input.just_pressed(KeyCode::KeyP) {
        point_cloud_render_mode.use_edl = !point_cloud_render_mode.use_edl;
    }
    if key_input.just_pressed(KeyCode::Numpad4) {
        point_cloud_render_mode.edl_neighbour_count = 4;
    }
    if key_input.just_pressed(KeyCode::Numpad8) {
        point_cloud_render_mode.edl_neighbour_count = 8;
    }
    if key_input.pressed(KeyCode::NumpadDivide) {
        point_cloud_render_mode.edl_radius -= 0.1;
    }
    if key_input.pressed(KeyCode::NumpadMultiply) {
        point_cloud_render_mode.edl_radius += 0.1;
    }
    if key_input.pressed(KeyCode::Numpad1) {
        point_cloud_render_mode.edl_strength -= 0.1;
    }
    if key_input.pressed(KeyCode::Numpad3) {
        point_cloud_render_mode.edl_strength += 0.1;
    }
}
