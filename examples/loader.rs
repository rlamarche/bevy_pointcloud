#[path = "helpers/camera_controller.rs"]
mod camera_controller;

use bevy::prelude::*;
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_pointcloud::new_potree::component::NewPotreePointCloud3d;
use bevy_pointcloud::new_potree::{PotreeAssetPlugin, PotreeServer, PotreeVisibilityPlugin};
use bevy_pointcloud::point_cloud_material::PointCloudMaterial;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins,
        PanOrbitCameraPlugin,
        PotreeAssetPlugin::default(),
        PotreeVisibilityPlugin::default(),
    ));

    app.add_systems(Startup, (setup, load_pointcloud)).run();
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
pub struct MyMaterial(Handle<PointCloudMaterial>);

fn load_pointcloud(mut commands: Commands, octree_server: Res<PotreeServer>) {
    let octree_handle = octree_server
        .load_octree("file:///home/romain/Documents/Potree/Messerschmitt".to_string());

    commands.spawn((
        NewPotreePointCloud3d(octree_handle),
        Transform::from_rotation(Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2)),
    ));
}
