#[path = "helpers/camera_controller.rs"]
mod camera_controller;

use bevy::prelude::*;
use bevy_diagnostic::DiagnosticsStore;
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_pointcloud::PointCloudPlugin;
use bevy_pointcloud::point_cloud_material::{PointCloudMaterial, PointCloudMaterial3d};
use bevy_pointcloud::pointcloud_octree::component::PointCloudOctree3d;
use bevy_pointcloud::pointcloud_octree::{
    PointCloudOctreePlugin, PointCloudOctreeVisibilityPlugin,
};
use bevy_pointcloud::potree::{PotreeServer, PotreeServerPlugin};
use bevy_pointcloud::render::PointCloudRenderMode;
use bevy_render::view::NoIndirectDrawing;
use std::ops::Mul;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins,
        PanOrbitCameraPlugin,
        PointCloudPlugin,
        PointCloudOctreePlugin,
        PotreeServerPlugin::default(),
    ));

    app.add_systems(Startup, (setup_ui, setup, load_pointcloud))
        .add_systems(Update, update_ui)
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
    octree_server: Res<PotreeServer>,
) {
    let my_material = point_cloud_materials.add(PointCloudMaterial {
        point_size: 30.0,
        min_point_size: 2.0,
        max_point_size: 50.0,
        ..default()
    });
    commands.spawn(MyMaterial(my_material.clone()));

    let octree_handle = octree_server.load_octree("https://pub-e2043f8abc6f45d983f8f77641ea772e.r2.dev/potree/heidentor".to_string());

    commands.spawn((
        PointCloudOctree3d(octree_handle),
        Transform::from_rotation(Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2)),
        PointCloudMaterial3d(my_material.clone()),
    ));
}
