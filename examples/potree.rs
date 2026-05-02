#[path = "helpers/camera_controller.rs"]
mod camera_controller;

use bevy::DefaultPlugins;
use bevy_app::prelude::*;
use bevy_asset::{Assets, Handle};
use bevy_camera::Camera3d;
use bevy_color::Color;
#[cfg(all(not(feature = "webgl"), not(feature = "webgpu")))]
use bevy_color::palettes::basic::{GREEN, RED};
#[cfg(all(not(feature = "webgl"), not(feature = "webgpu")))]
use bevy_dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig};
use bevy_diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy_ecs::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use bevy_gizmos::prelude::*;
use bevy_math::prelude::*;
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_pointcloud::PointCloudPlugin;
use bevy_pointcloud::octree::visibility::components::{
    SkipOctreeVisibility, ViewVisibleOctreeNodes,
};
use bevy_pointcloud::point_cloud_material::{PointCloudMaterial, PointCloudMaterial3d};
use bevy_pointcloud::pointcloud_octree::asset::PointCloudOctree;
use bevy_pointcloud::pointcloud_octree::asset::data::PointCloudNodeData;
use bevy_pointcloud::pointcloud_octree::component::PointCloudOctree3d;
use bevy_pointcloud::pointcloud_octree::{
    ExtractVisiblePointCloudOctreeNodesPlugin, PointCloudOctreePlugin, PointCloudOctreeServer,
    PointCloudOctreeServerPlugin, PointCloudOctreeVisibilityPlugin,
    PointCloudOctreeVisibilitySettings,
};
use bevy_pointcloud::potree::loader::PotreeLoader;
use bevy_pointcloud::render::PointCloudRenderMode;
use bevy_render::prelude::*;
use bevy_text::{FontSmoothing, TextFont};
use bevy_transform::prelude::*;
use bevy_utils::default;
use bevy_window::{PresentMode, Window};
use potree::asset::fs::PotreeFsAsset;
use std::ops::Mul;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins,
        EguiPlugin::default(),
        // WorldInspectorPlugin::default(),
        PanOrbitCameraPlugin,
        PointCloudPlugin,
        PointCloudOctreePlugin.set(ExtractVisiblePointCloudOctreeNodesPlugin::with_max_size(
            // limit to 1 mb of gpu memory
            512 * 1024 * 1024,
        )),
        PointCloudOctreeServerPlugin::with_max_size(
            // limit to 512 mb of cpu memory
            512 * 1024 * 1024,
        ),
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

    app.add_systems(Startup, (setup_window, setup, load_pointcloud))
        // .add_systems(PreUpdate, draw_gizmos.after(propagate_parent_transforms))
        .add_systems(EguiPrimaryContextPass, ui_settings)
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
        Transform::from_xyz(18.0, 8.0, -3.0).looking_at(Vec3::new(0.0, 8.0, -3.0), Vec3::Y),
        PanOrbitCamera::default(),
        Msaa::Off,
        PointCloudRenderMode {
            use_edl: true,
            edl_radius: 1.4,
            edl_strength: 0.4,
            edl_neighbour_count: 4,
            ..Default::default()
        },
        PointCloudOctreeVisibilitySettings {
            filter: Some(30.0),
            budget: Some(1_000_000),
        },
    ));
}

#[derive(Component)]
pub struct MyMaterial(#[allow(unused)] Handle<PointCloudMaterial>);

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

    let octree_handle = octree_server
        .load_octree::<PotreeLoader<_>>(PotreeFsAsset::from_path("assets/potree/heidentor"));

    commands.spawn((
        PointCloudOctree3d(octree_handle),
        Transform::from_rotation(Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2)),
        PointCloudMaterial3d(my_material.clone()),
    ));
}

#[allow(unused)]
fn draw_gizmos(
    octrees: Res<Assets<PointCloudOctree>>,
    entities: Query<&GlobalTransform, With<PointCloudOctree3d>>,
    octrees_vibility: Query<&ViewVisibleOctreeNodes<PointCloudNodeData, PointCloudOctree3d>>,
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

                gizmos.cube(world_transform, RED);
            }
        }
    }
}

fn ui_settings(
    mut commands: Commands,
    mut contexts: EguiContexts,
    mut point_cloud_settings: Query<(
        Entity,
        &mut PointCloudOctreeVisibilitySettings,
        &mut PointCloudRenderMode,
        Option<&SkipOctreeVisibility>,
    )>,
    diagnostic: Res<DiagnosticsStore>,
) -> Result {
    let (view_entity, mut point_cloud_settings, mut point_cloud_render_mode, skip_visibility_check) =
        point_cloud_settings.single_mut().unwrap();

    let mut use_edl = point_cloud_render_mode.use_edl;
    let mut edl_radius = point_cloud_render_mode.edl_radius;
    let mut edl_strength = point_cloud_render_mode.edl_strength;
    let mut edl_neighbour_count = point_cloud_render_mode.edl_neighbour_count;
    let mut min_node_size: f32 = point_cloud_settings.filter.unwrap_or(0.0);
    let mut point_budget: usize = point_cloud_settings.budget.unwrap_or(0);

    let mut has_skip_visibility_check = skip_visibility_check.is_some();

    let visibility_time = diagnostic
        .get(&PointCloudOctreeVisibilityPlugin::VISIBILITY_CHECK_TIME)
        .and_then(|value| value.smoothed());

    let nb_points = diagnostic
        .get(&PointCloudOctreeVisibilityPlugin::BUDGET)
        .and_then(|value| value.smoothed());

    let fps = diagnostic
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|value| value.smoothed());

    egui::Window::new("Settings & Statistics")
        .auto_sized()
        .show(contexts.ctx_mut()?, |ui| {
            ui.vertical(|ui| {
                ui.label("Render Settings");
                ui.checkbox(&mut has_skip_visibility_check, "Skip Visibility Check");
                ui.checkbox(&mut use_edl, "Eye Dome Lightning");
                ui.add(
                    egui::Slider::new(&mut edl_radius, 0.0..=10.0)
                        .text("EDL radius")
                        .step_by(0.01)
                        .drag_value_speed(0.01),
                );
                ui.add(
                    egui::Slider::new(&mut edl_strength, 0.0..=10.0)
                        .text("EDL strength")
                        .step_by(0.01)
                        .drag_value_speed(0.01),
                );
                ui.add(
                    egui::Slider::new(&mut edl_neighbour_count, 4..=8)
                        .text("EDL neighbour count")
                        .step_by(4.0)
                        .drag_value_speed(4.0),
                );

                ui.label("Budget Settings");
                ui.add(
                    egui::Slider::new(&mut min_node_size, 30.0..=1000.0)
                        .text("Min node size")
                        .step_by(10.0)
                        .drag_value_speed(10.0),
                );
                ui.add(
                    egui::Slider::new(&mut point_budget, 100_000..=10_000_000)
                        .text("Point budget")
                        .step_by(1000.0)
                        .drag_value_speed(1000.0),
                );

                ui.label("Statistics");

                if let Some(fps) = fps {
                    ui.horizontal(|ui| {
                        ui.label("FPS: ");
                        ui.label(format!("{fps:.0}"))
                    });
                };

                if let Some(visibility_time) = visibility_time {
                    ui.horizontal(|ui| {
                        ui.label("Visibility time: ");
                        ui.label(format!("{visibility_time:.2}ms"))
                    });
                };

                if let Some(nb_points) = nb_points {
                    let nb_points = nb_points as usize;
                    ui.horizontal(|ui| {
                        ui.label("Nb points: ");
                        ui.label(format!("{nb_points}"))
                    });
                };
            });
        });

    point_cloud_render_mode.use_edl = use_edl;
    point_cloud_render_mode.edl_radius = edl_radius;
    point_cloud_render_mode.edl_strength = edl_strength;
    point_cloud_render_mode.edl_neighbour_count = edl_neighbour_count;

    point_cloud_settings.filter = Some(min_node_size);
    point_cloud_settings.budget = Some(point_budget);

    if has_skip_visibility_check != skip_visibility_check.is_some() {
        match has_skip_visibility_check {
            true => {
                commands.entity(view_entity).insert(SkipOctreeVisibility);
            }
            false => {
                commands
                    .entity(view_entity)
                    .remove::<SkipOctreeVisibility>();
            }
        }
    }

    Ok(())
}
