#![expect(missing_docs, reason = "Not all docs are written yet.")]

use bevy::{
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    prelude::*,
    remote::{http::RemoteHttpPlugin, RemotePlugin},
    render::view::NoIndirectDrawing,
};
use bevy_pointcloud::{las::LasLoader, prelude::*, FileSource, PointCloud3d, PointCloudServer};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins((RemotePlugin::default(), RemoteHttpPlugin::default()))
        .add_plugins(FreeCameraPlugin)
        .add_plugins(PointCloudPlugin::default())
        .add_systems(Startup, (setup, load_point_cloud))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 5.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
        NoIndirectDrawing,
        Msaa::Off,
        FreeCamera::default(),
    ));
}

fn load_point_cloud(point_cloud_server: Res<PointCloudServer>, mut commands: Commands) -> Result {
    let loader = LasLoader::from(FileSource::open(
        "assets/pointclouds/lion_takanawa.copc.laz",
    )?);

    let point_cloud_handle = point_cloud_server.load(loader);
    commands.spawn((PointCloud3d(point_cloud_handle),));

    Ok(())
}
