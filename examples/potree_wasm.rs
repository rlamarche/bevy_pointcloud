#[path = "helpers/camera_controller.rs"]
mod camera_controller;

use bevy::prelude::*;
use bevy_diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_pointcloud::PointCloudPlugin;
use bevy_pointcloud::point_cloud_material::{PointCloudMaterial, PointCloudMaterial3d};
use bevy_pointcloud::pointcloud_octree::component::PointCloudOctree3d;
use bevy_pointcloud::pointcloud_octree::{
    ExtractVisiblePointCloudOctreeNodesPlugin, PointCloudOctreePlugin, PointCloudOctreeServer,
    PointCloudOctreeServerPlugin, PointCloudOctreeVisibilityPlugin,
    PointCloudOctreeVisibilitySettings,
};
use bevy_pointcloud::potree::loader::PotreeLoader;
use bevy_pointcloud::render::PointCloudRenderMode;
use bevy_render::view::NoIndirectDrawing;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins,
        EguiPlugin::default(),
        PanOrbitCameraPlugin,
        PointCloudPlugin,
        PointCloudOctreePlugin.set(ExtractVisiblePointCloudOctreeNodesPlugin::with_max_size(
            // limit to 256 mb of gpu memory
            256 * 1024 * 1024,
        )),
        PointCloudOctreeServerPlugin::with_max_size(
            // limit to 512 mb of cpu memory
            512 * 1024 * 1024,
        ),
    ));

    app.add_systems(Startup, (setup, load_pointcloud))
        .add_systems(EguiPrimaryContextPass, ui_settings)
        .run();
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
        PointCloudOctreeVisibilitySettings {
            filter: Some(150.0),
            budget: Some(1_000_000),
        },
    ));
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

    // let octree_handle = octree_server.load_octree::<PotreeLoader>(
    //     "https://pub-e2043f8abc6f45d983f8f77641ea772e.r2.dev/potree/heidentor",
    // );

    let octree_handle = octree_server.load_octree::<PotreeLoader>("assets/potree/heidentor");

    commands.spawn((
        PointCloudOctree3d(octree_handle),
        Transform::from_rotation(Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2)),
        PointCloudMaterial3d(my_material.clone()),
    ));
}

fn ui_settings(
    mut contexts: EguiContexts,
    mut point_cloud_settings: Query<(
        &mut PointCloudOctreeVisibilitySettings,
        &mut PointCloudRenderMode,
    )>,
    diagnostic: Res<DiagnosticsStore>,
) -> Result {
    let (mut point_cloud_settings, mut point_cloud_render_mode) =
        point_cloud_settings.single_mut().unwrap();

    let mut use_edl = point_cloud_render_mode.use_edl;
    let mut edl_radius = point_cloud_render_mode.edl_radius;
    let mut edl_strength = point_cloud_render_mode.edl_strength;
    let mut edl_neighbour_count = point_cloud_render_mode.edl_neighbour_count;
    let mut min_node_size: f32 = point_cloud_settings.filter.unwrap_or(0.0);
    let mut point_budget: usize = point_cloud_settings.budget.unwrap_or(0);

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
                    egui::Slider::new(&mut min_node_size, 50.0..=1000.0)
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

    Ok(())
}
