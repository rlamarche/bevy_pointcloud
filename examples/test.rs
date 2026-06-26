#![expect(missing_docs, reason = "Not all docs are written yet.")]
use bevy::{
    asset::RenderAssetUsages, camera::visibility::NoFrustumCulling, mesh::PrimitiveTopology,
    prelude::*, render::view::NoIndirectDrawing,
};
use bevy_camera_controller::free_camera::{FreeCamera, FreeCameraPlugin};
use bevy_pointcloud::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FreeCameraPlugin)
        .add_plugins(PointCloudPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let mut points = Vec::new();
    for x in -5..=5 {
        for y in -5..=5 {
            for z in -5..=5 {
                points.push([x as f32, y as f32, z as f32]);
            }
        }
    }

    let points_mesh = meshes.add(
        Mesh::new(PrimitiveTopology::PointList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, points),
    );

    let quad_mesh = meshes.add(Rectangle::new(1.0, 1.0));

    commands.spawn((
        PointCloud {
            quad_mesh,
            points_mesh,
        },
        NoFrustumCulling,
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 5.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
        NoIndirectDrawing,
        Msaa::Off,
        FreeCamera::default(),
    ));
}
