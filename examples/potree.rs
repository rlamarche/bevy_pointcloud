#[path = "helpers/camera_controller.rs"]
mod camera_controller;

use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig};
use bevy::prelude::*;
use bevy::text::FontSmoothing;
use bevy::window::PresentMode;
use bevy_color::palettes::basic::{GREEN, RED};
use bevy_diagnostic::DiagnosticsStore;
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_pointcloud::PointCloudPlugin;
use bevy_pointcloud::octree::visibility::components::OctreesVisibility;
use bevy_pointcloud::point_cloud_material::{PointCloudMaterial, PointCloudMaterial3d};
use bevy_pointcloud::pointcloud_octree::asset::PointCloudOctree;
use bevy_pointcloud::pointcloud_octree::asset::data::PointCloudNodeData;
use bevy_pointcloud::pointcloud_octree::component::PointCloudOctree3d;
use bevy_pointcloud::pointcloud_octree::{PointCloudOctreePlugin, PointCloudOctreeServer, PointCloudOctreeServerPlugin, PointCloudOctreeVisibilityPlugin};
use bevy_pointcloud::render::PointCloudRenderMode;
use bevy_render::view::NoIndirectDrawing;
use std::ops::Mul;
use bevy_pointcloud::potree::loader::PotreeLoader;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins,
        PanOrbitCameraPlugin,
        PointCloudPlugin,
        PointCloudOctreePlugin,
        PointCloudOctreeServerPlugin::default(),
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

    app.add_systems(Startup, (setup_window, setup_ui, setup, load_pointcloud))
        // .add_systems(PreUpdate, draw_gizmos.after(propagate_parent_transforms))
        .add_systems(Update, update_ui)
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
        PanOrbitCamera::default(),
        Msaa::Off,
        PointCloudRenderMode {
            use_edl: true,
            edl_radius: 1.4,
            edl_strength: 0.4,
            edl_neighbour_count: 4,
            ..Default::default()
        },
    ));
}

#[derive(Component)]
struct TimedText;

fn setup_ui(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                bottom: percent(1.0),
                right: percent(1.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("VISIBILITY TIME: "),
                TextColor(Color::WHITE.into()),
                TimedText,
                Pickable::IGNORE,
            ))
            .with_child(TextSpan::default());
        });
}

fn update_ui(
    diagnostic: Res<DiagnosticsStore>,
    mut writer: TextUiWriter,
    query: Query<Entity, With<TimedText>>,
) {
    for entity in &query {
        if let Some(time) = diagnostic.get(&PointCloudOctreeVisibilityPlugin::VISIBILITY_CHECK_TIME)
            && let Some(value) = time.smoothed()
        {
            *writer.text(entity, 1) = format!("{value:.2}");
        }
    }
}

#[derive(Component)]
pub struct MyMaterial(Handle<PointCloudMaterial>);

fn load_pointcloud(
    mut commands: Commands,
    mut point_cloud_materials: ResMut<Assets<PointCloudMaterial>>,
    octree_server: Res<PointCloudOctreeServer>,
) {
    let my_material = point_cloud_materials.add(PointCloudMaterial {
        point_size: 30.0,
        min_point_size: 2.0,
        max_point_size: 50.0,
        ..default()
    });
    commands.spawn(MyMaterial(my_material.clone()));

    let octree_handle = octree_server.load_octree::<PotreeLoader>("assets/potree/heidentor".to_string());

    commands.spawn((
        PointCloudOctree3d(octree_handle),
        Transform::from_rotation(Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2)),
        PointCloudMaterial3d(my_material.clone()),
    ));
}

fn draw_gizmos(
    octrees: Res<Assets<PointCloudOctree>>,
    entities: Query<&GlobalTransform, With<PointCloudOctree3d>>,
    octrees_vibility: Query<&OctreesVisibility<PointCloudNodeData, PointCloudOctree3d>>,
    mut gizmos: Gizmos,
) {
    for octree_visibility in octrees_vibility {
        for (entity, (asset_id, visible_nodes)) in &octree_visibility.octrees {
            let Ok(global_transform) = entities.get(*entity) else {
                continue;
            };
            let Some(octree) = octrees.get(*asset_id) else {
                continue;
            };

            for visible_node in visible_nodes {
                let Some(node) = octree.hierarchy_node(visible_node.id) else {
                    continue;
                };

                let center = node.bounding_box.center.clone();
                let scale = node.bounding_box.half_extents.mul(2.0);

                let local_transform =
                    Transform::from_translation(center.into()).with_scale(scale.into());

                let world_transform = global_transform.mul_transform(local_transform);

                gizmos.cuboid(world_transform, RED);
            }
        }
    }
}
