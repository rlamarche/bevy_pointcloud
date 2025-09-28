#[path = "helpers/camera_controller.rs"]
mod camera_controller;

use crate::camera_controller::{CameraController, CameraControllerPlugin};
use bevy::tasks::futures_lite::future;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on};
use bevy::{prelude::*, render::view::NoIndirectDrawing};
use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin};
use bevy::text::FontSmoothing;
#[cfg(all(not(feature = "webgl"), not(feature = "webgpu")))]
use bevy::window::PresentMode;
use bevy_color::palettes::basic::{GREEN, RED};
use bevy_ecs::world::CommandQueue;
use bevy_math::bounding::BoundingVolume;
use bevy_panorbit_camera::PanOrbitCameraPlugin;
use bevy_pointcloud::PointCloudPlugin;
use bevy_pointcloud::point_cloud::PointCloud;
use bevy_pointcloud::point_cloud_material::PointCloudMaterial;
use bevy_pointcloud::potree::prelude::*;
use bevy_pointcloud::render::PointCloudRenderMode;

use bevy_render::primitives::{Aabb, Frustum};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PanOrbitCameraPlugin,
            PointCloudPlugin,
            CameraControllerPlugin,
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
        .add_systems(Update, (handle_tasks, update_point_cloud_visibility))
        .run();
}

fn setup_window(mut windows: Query<&mut Window>) {
    let mut window = windows.single_mut().unwrap();

    #[cfg(all(not(feature = "webgl"), not(feature = "webgpu")))]
    {
        window.present_mode = PresentMode::Immediate;
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

#[derive(Component)]
struct MyPotreePointCloud(Handle<PotreePointCloud>);

#[derive(Component)]
struct ComputeTransform(Task<CommandQueue>);

#[derive(Component)]
struct MyGizmo;

#[derive(Component, Deref)]
struct MyPointCloud(pub potree::point_cloud::PotreePointCloud);

fn load_pointcloud(
    mut commands: Commands,
    mut point_cloud_materials: ResMut<Assets<PointCloudMaterial>>,
    mut point_clouds: ResMut<Assets<PointCloud>>,
    asset_server: Res<AssetServer>,
    mut gizmo_assets: ResMut<Assets<GizmoAsset>>,
) {
    let potree_point_cloud_handle: Handle<PotreePointCloud> = asset_server.load("potree/heidentor/metadata.json");
    commands.spawn((
        PotreePointCloud3d {
            handle: potree_point_cloud_handle,
        },
        DrawPotreeGizmo,
        Transform::from_rotation(Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2)),
    ));

    // Create a gizmo with our transform
    // let gizmo = GizmoAsset::new();
    // let gizmo_handle = gizmo_assets.add(gizmo);
    //
    // commands.spawn((
    //     MyGizmo,
    //     Gizmo {
    //         handle: gizmo_handle,
    //         line_config: GizmoLineConfig {
    //             width: 1.,
    //             ..default()
    //         },
    //         ..default()
    //     },
    //     Transform::from_rotation(Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2)),
    // ));

    return;

    let my_material = point_cloud_materials.add(PointCloudMaterial {
        point_size: 30.0,
        ..default()
    });
    commands.spawn(MyMaterial(my_material.clone()));

    // load the potree data in an async thread
    let thread_pool = AsyncComputeTaskPool::get();
    let entity = commands.spawn_empty().id();
    let task = thread_pool.spawn(async move {
        let mut potree_point_cloud = potree::point_cloud::PotreePointCloud::from_url(
            "file:///home/romain/code/bevy/potree-rs/assets/heidentor",
            potree::resource::ResourceLoader::new(),
        )
        .await
        .expect("Error loading pointcloud");

        potree_point_cloud
            .load_entire_hierarchy()
            .await
            .expect("Error loading entire hierarchy");

        let hierarchy_snapshot = potree_point_cloud.hierarchy_snapshot();

        // info!("Hierarchy snapshot: {:#?}", hierarchy_snapshot);

        let mut command_queue = CommandQueue::default();

        command_queue.push(move |world: &mut World| {
            let mut gizmo = GizmoAsset::new();

            hierarchy_snapshot.iter().for_each(|node| {
                let center = (node.bounding_box.min + node.bounding_box.max) / 2.0;
                let scale = node.bounding_box.max - node.bounding_box.min;
                gizmo.cuboid(
                    Transform::from_translation(Vec3::new(
                        center.x as f32,
                        center.y as f32,
                        center.z as f32,
                    ))
                    .with_scale(Vec3::new(
                        scale.x as f32,
                        scale.y as f32,
                        scale.z as f32,
                    )),
                    RED,
                );
            });

            let mut gizmo_assets = world.resource_mut::<Assets<GizmoAsset>>();
            let gizmo_handle = gizmo_assets.add(gizmo);

            let mut commands = world.commands();

            commands.spawn(MyPointCloud(potree_point_cloud));

            commands.spawn((
                MyGizmo,
                Gizmo {
                    handle: gizmo_handle,
                    line_config: GizmoLineConfig {
                        width: 1.,
                        ..default()
                    },
                    ..default()
                },
                Transform::from_rotation(Quat::from_axis_angle(
                    Vec3::X,
                    -std::f32::consts::FRAC_PI_2,
                )),
            ));
        });

        command_queue
    });
    commands.entity(entity).insert(ComputeTransform(task));

    // let point_cloud = asset_server.load::<PointCloud>("potree/heidentor/metadata.json");
    // commands.spawn((
    //     PointCloud3d(point_cloud),
    //     PointCloudMaterial3d(my_material.clone()),
    //     MainPointCloud,
    // ));
}

/// This system queries for entities that have our Task<Transform> component. It polls the
/// tasks to see if they're complete. If the task is complete it takes the result, adds a
/// new [`Mesh3d`] and [`MeshMaterial3d`] to the entity using the result from the task's work, and
/// removes the task component from the entity.
fn handle_tasks(
    mut commands: Commands,
    mut transform_tasks: Query<(Entity, &mut ComputeTransform)>,
) {
    for (entity, mut task) in &mut transform_tasks {
        if let Some(mut commands_queue) = block_on(future::poll_once(&mut task.0)) {
            // append the returned command queue to have it execute later
            commands.append(&mut commands_queue);
            commands.entity(entity).despawn()
        }
    }
}

fn compute_visibility<'a>(
    node: &'a potree::octree::snapshot::OctreeNodeSnapshot,
    transform: &GlobalTransform,
    frustum: &Frustum,
) -> Vec<&'a potree::octree::snapshot::OctreeNodeSnapshot> {
    let min = node.bounding_box.min;
    let max = node.bounding_box.max;

    let model_aabb = Aabb::from_min_max(
        Vec3::new(min.x as f32, min.y as f32, min.z as f32),
        Vec3::new(max.x as f32, max.y as f32, max.z as f32),
    );

    let world_from_local = transform.affine();

    let model_sphere = bevy::render::primitives::Sphere {
        center: world_from_local.transform_point3a(model_aabb.center),
        radius: transform.radius_vec3a(model_aabb.half_extents),
    };

    // Do quick sphere-based frustum culling
    if !frustum.intersects_sphere(&model_sphere, false) {
        return vec![];
    }

    if (frustum.contains_aabb(&model_aabb, &world_from_local)) {
        // if it is completely contained, return all nodes recursively
        // return vec![node];
        return std::iter::once(node).chain(node.iter()).collect();
    }

    // Do aabb-based frustum culling
    if !frustum.intersects_obb(&model_aabb, &world_from_local, true, false) {
        return vec![];
    }

    // Recursively check children
    node.children
        .iter()
        .flat_map(|child| compute_visibility(child, transform, frustum))
        .collect::<Vec<_>>()
}

fn update_point_cloud_visibility(
    mut gizmo_assets: ResMut<Assets<GizmoAsset>>,
    gizmo: Query<(&Gizmo, &GlobalTransform), With<MyGizmo>>,
    mut commands: Commands,
    point_cloud: Query<&mut MyPointCloud>,
    key_input: Res<ButtonInput<KeyCode>>,
    frustums: Query<&Frustum, With<Camera>>,
    my_potree_point_cloud: Query<&MyPotreePointCloud>,
    potree_assets: Res<Assets<PotreePointCloud>>,
    mut gizmos: Gizmos,
) {
    // let Ok(frustum) = frustums.single() else {
    //     warn!("No frustum.");
    //     return;
    // };
    //
    // let Ok(my_point_cloud) = my_potree_point_cloud.single() else {
    //     warn!("My point cloud not yet available");
    //     return;
    // };
    //
    // let Some(potree_point_cloud_asset) = potree_assets.get(&my_point_cloud.0) else {
    //     warn!("Missing Potree Point Cloud in assets");
    //     return;
    // };
    //
    // let Ok((gizmo, gizmo_transform)) = gizmo.single() else {
    //     warn!("Gizmo not available.");
    //     return;
    // };
    //
    // let hierarchy_snapshot = potree_point_cloud_asset.wrapped.hierarchy_snapshot();
    //
    // let mut gizmo_asset = gizmo_assets
    //     .get_mut(&gizmo.handle)
    //     .expect("Gizmo asset not found");
    //
    // let nodes = compute_visibility(&hierarchy_snapshot, &gizmo_transform, &frustum);
    //
    // gizmo_asset.clear();
    // for node in nodes {
    //     let center = (node.bounding_box.min + node.bounding_box.max) / 2.0;
    //     let scale = node.bounding_box.max - node.bounding_box.min;
    //     gizmo_asset.cuboid(
    //         Transform::from_translation(Vec3::new(
    //             center.x as f32,
    //             center.y as f32,
    //             center.z as f32,
    //         ))
    //         .with_scale(Vec3::new(scale.x as f32, scale.y as f32, scale.z as f32)),
    //         RED,
    //     );
    // }
}
