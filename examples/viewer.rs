mod ui;

use std::ops::Neg;

use bevy::{
    app::prelude::*,
    asset::Assets,
    camera::{visibility::Visibility, Camera3d},
    ecs::prelude::*,
    feathers::{dark_theme::create_dark_theme, theme::UiTheme, FeathersPlugins},
    input_focus::InputDispatchPlugin,
    math::prelude::*,
    transform::prelude::*,
    DefaultPlugins,
};
use bevy_asset::AssetEvent;
use bevy_camera::{primitives::Aabb, Projection};
use bevy_log::info;
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_pointcloud::{
    octree_loader::copc::{CopcLoader, HttpSource},
    point_cloud_material::{PointCloudMaterial, PointCloudMaterial3d},
    pointcloud_octree::{
        asset::PointCloudOctree, component::PointCloudOctree3d,
        ExtractVisiblePointCloudOctreeNodesPlugin, PointCloudOctreePlugin, PointCloudOctreeServer,
        PointCloudOctreeServerPlugin, PointCloudOctreeVisibilitySettings,
    },
    render::PointCloudRenderMode,
    PointCloudPlugin,
};
use bevy_render::prelude::*;
use bevy_ui_text_input::TextInputPlugin;

use crate::ui::{LoadPointCloudMessage, MyMaterial, MyUiPlugin, PointCloudRoot, UiState};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DEFAULT_URL: &str = "https://s3.amazonaws.com/hobu-lidar/autzen-classified.copc.laz";

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let mut app = App::new();

    app.add_plugins((
        DefaultPlugins,
        FeathersPlugins.build().disable::<InputDispatchPlugin>(),
        TextInputPlugin,
        PanOrbitCameraPlugin,
        PointCloudPlugin,
        PointCloudOctreePlugin.set(ExtractVisiblePointCloudOctreeNodesPlugin::with_max_size(
            512 * 1024 * 1024, // 512 MB GPU budget
        )),
        PointCloudOctreeServerPlugin::with_max_size(
            1024 * 1024 * 1024, // 1 GB CPU budget
        ),
        MyUiPlugin,
    ));

    app.insert_resource(UiTheme(create_dark_theme()))
        .add_systems(Startup, (setup_camera, load_initial_pointcloud))
        .add_systems(
            PreUpdate,
            (handle_url_submit, update_camera_control, center_point_cloud),
        )
        .add_systems(PostUpdate, move_octree_at_center)
        .run();
}

// ---------------------------------------------------------------------------
// Startup systems
// ---------------------------------------------------------------------------

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        // Transform::from_xyz(18.0, 8.0, -3.0).looking_at(Vec3::new(0.0, 8.0, -3.0), Vec3::Y),
        PanOrbitCamera::default(),
        Msaa::Off,
        PointCloudRenderMode {
            use_edl: true,
            edl_radius: 1.4,
            edl_strength: 0.4,
            edl_neighbour_count: 4,
        },
        PointCloudOctreeVisibilitySettings {
            filter: Some(30.0),
            budget: Some(1_000_000),
        },
    ));
}

// ---------------------------------------------------------------------------
// UI Behavior
// ---------------------------------------------------------------------------

fn update_camera_control(mut cameras: Query<&mut PanOrbitCamera>, ui_state: Res<UiState>) {
    for mut cam in &mut cameras {
        cam.enabled = !ui_state.dragging && !ui_state.hovering;
    }
}

// ---------------------------------------------------------------------------
// Loading of a point cloud
// ---------------------------------------------------------------------------

#[derive(Component)]
struct LoadingPointCloud;

fn load_initial_pointcloud(
    mut commands: Commands,
    mut materials: ResMut<Assets<PointCloudMaterial>>,
    octree_server: Res<PointCloudOctreeServer>,
) {
    spawn_pointcloud(&mut commands, &mut materials, &octree_server, DEFAULT_URL);
}

/// React to `LoadPointCloudEvent`: despawn old cloud, spawn new one.
fn handle_url_submit(
    mut commands: Commands,
    mut events: MessageReader<LoadPointCloudMessage>,
    mut materials: ResMut<Assets<PointCloudMaterial>>,
    octree_server: Res<PointCloudOctreeServer>,
    roots: Query<Entity, With<PointCloudRoot>>,
) {
    for event in events.read() {
        // Despawn every existing cloud root (there should be at most one).
        for entity in &roots {
            info!("Despawn entity {}", entity);
            commands.entity(entity).despawn();
        }

        spawn_pointcloud(&mut commands, &mut materials, &octree_server, &event.url);
    }
}

/// Shared logic for (re-)spawning a point cloud from a URL.
fn spawn_pointcloud(
    commands: &mut Commands,
    materials: &mut ResMut<Assets<PointCloudMaterial>>,
    octree_server: &Res<PointCloudOctreeServer>,
    url: &str,
) -> Option<Entity> {
    let material = materials.add(PointCloudMaterial {
        point_size: 30.0,
        min_point_size: 2.0,
        max_point_size: 50.0,
    });

    // Keep the material handle alive on a dedicated entity.
    commands.spawn(MyMaterial(material.clone()));

    let Ok(source) = HttpSource::open(url) else {
        bevy_log::warn!("Invalid URL: {url}");
        return None;
    };

    let octree_handle = octree_server.load_octree::<CopcLoader<_>>(source);

    Some(
        commands
            .spawn((
                PointCloudRoot,
                Transform::from_rotation(Quat::from_axis_angle(
                    Vec3::X,
                    -std::f32::consts::FRAC_PI_2,
                )),
                Visibility::default(),
                children![(
                    LoadingPointCloud,
                    PointCloudOctree3d(octree_handle),
                    PointCloudMaterial3d(material),
                )],
            ))
            .id(),
    )
}

#[allow(clippy::type_complexity)]
fn move_octree_at_center(
    mut events: MessageReader<AssetEvent<PointCloudOctree>>,
    assets: Res<Assets<PointCloudOctree>>,
    loading_point_clouds: Query<(Entity, &PointCloudOctree3d, &Aabb), With<LoadingPointCloud>>,
    mut camera: Query<
        (&mut Transform, &mut PanOrbitCamera, &Projection),
        (With<Camera3d>, Without<PointCloudOctree3d>),
    >,
    mut commands: Commands,
) {
    for event in events.read() {
        if let AssetEvent::Added { id } = event {
            let Some(asset) = assets.get(*id) else {
                return;
            };

            let Some(root) = asset.hierarchy_root() else {
                return;
            };

            for (entity, PointCloudOctree3d(handle), aabb) in loading_point_clouds {
                if handle.id().eq(id) {
                    commands
                        .entity(entity)
                        .insert(Transform::from_translation(
                            root.bounding_box.center.neg().into(),
                        ))
                        .remove::<LoadingPointCloud>();

                    let (mut camera_transform, mut pan_orbit_camera, projection) =
                        camera.single_mut().unwrap();

                    fit_camera_to_centered_aabb(
                        &mut camera_transform,
                        projection,
                        &aabb.half_extents.into(),
                        Vec3::new(1.0, 1.0, 1.0), // vue isométrique-ish
                    );

                    // recompute camera yaw/pitch/radius
                    let target_focus = Vec3::new(0.0, 0.0, 0.0);
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
        }
    }
}

/// Makes camera looking at AABB
fn fit_camera_to_centered_aabb(
    camera_transform: &mut Transform,
    projection: &Projection,
    half_extents: &Vec3,
    direction: Vec3,
) {
    let bounding_radius = half_extents.length();

    let dir = direction.normalize();

    let distance = match projection {
        Projection::Perspective(persp) => {
            let fov_y = persp.fov;
            bounding_radius / (fov_y * 0.5).tan()
        }
        Projection::Orthographic(_) => bounding_radius * 2.0,
        _ => panic!("Custom project not supported"),
    };

    camera_transform.translation = dir * distance;

    let up = if dir.dot(Vec3::Y).abs() > 0.99 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    *camera_transform =
        Transform::from_translation(camera_transform.translation).looking_at(Vec3::ZERO, up);
}

#[allow(clippy::type_complexity)]
fn center_point_cloud(
    mut camera: Query<
        (&mut Transform, &mut PanOrbitCamera),
        (With<Camera3d>, Without<PointCloudOctree3d>),
    >,
    mut query: Query<(&Aabb, &mut Transform), (With<PointCloudOctree3d>, Changed<Aabb>)>,
) {
    let Some((aabb, mut transform)) = query.iter_mut().next() else {
        return;
    };

    info!("Center point cloud");

    // Center point cloud
    *transform = Transform::from_translation(
        (aabb.center.neg() + Vec3A::new(0.0, aabb.half_extents.y, 0.0)).into(),
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
