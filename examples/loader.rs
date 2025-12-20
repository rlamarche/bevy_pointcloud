#[path = "helpers/camera_controller.rs"]
mod camera_controller;

use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig};
use bevy::diagnostic;
use bevy::prelude::*;
use bevy::text::FontSmoothing;
use bevy::window::PresentMode;
use bevy_color::palettes::basic::{GREEN, RED};
use bevy_diagnostic::{DiagnosticPath, Diagnostics, DiagnosticsStore};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_pointcloud::new_potree::component::NewPotreePointCloud3d;
use bevy_pointcloud::new_potree::loader::PotreeHierarchy;
use bevy_pointcloud::new_potree::{PotreeAsset, PotreeAssetPlugin, PotreeExtractVisibleOctreeNodesPlugin, PotreeServer, PotreeServerPlugin, PotreeVisibilityPlugin};
use bevy_pointcloud::octree::new_asset::visibility::components::OctreesVisibility;
use bevy_pointcloud::point_cloud_material::PointCloudMaterial;
use bevy_pointcloud::pointcloud_octree::asset::PointCloudNodeData;
use bevy_transform::systems::propagate_parent_transforms;
use std::ops::Mul;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins,
        PanOrbitCameraPlugin,
        PotreeAssetPlugin::default(),
        PotreeServerPlugin::default(),
        PotreeVisibilityPlugin::default(),
        // PotreeExtractVisibleOctreeNodesPlugin::default(),
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
        .add_systems(PreUpdate, draw_gizmos.after(propagate_parent_transforms))
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
        PanOrbitCamera::default(),
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
        if let Some(time) = diagnostic.get(&PotreeVisibilityPlugin::VISIBILITY_CHECK_TIME)
            && let Some(value) = time.smoothed()
        {
            *writer.text(entity, 1) = format!("{value:.2}");
        }
    }
}

#[derive(Component)]
pub struct MyMaterial(Handle<PointCloudMaterial>);

fn load_pointcloud(mut commands: Commands, octree_server: Res<PotreeServer>) {
    let octree_handle =
        octree_server.load_octree("file:///home/romain/Documents/Potree/Messerschmitt".to_string());

    commands.spawn((
        NewPotreePointCloud3d(octree_handle),
        Transform::from_rotation(Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2)),
    ));
}

fn draw_gizmos(
    octrees: Res<Assets<PotreeAsset>>,
    entities: Query<&GlobalTransform, With<NewPotreePointCloud3d>>,
    octrees_vibility: Query<
        &OctreesVisibility<PotreeHierarchy, PointCloudNodeData, NewPotreePointCloud3d>,
    >,
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
