#![expect(missing_docs, reason = "Not all docs are written yet.")]

use bevy::{
    camera::primitives::Aabb,
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    prelude::*,
};
use bevy_pointcloud::{las::LasLoader, prelude::*, ColorStop};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FreeCameraPlugin)
        .add_plugins(PointCloudPlugin::default())
        .add_systems(Startup, (setup, load_point_cloud))
        .add_systems(PostUpdate, update_material_gradient)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(2.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        FreeCamera::default(),
    ));
}

fn load_point_cloud(
    mut materials: ResMut<Assets<SimplePointCloudMaterial>>,
    point_cloud_server: Res<PointCloudServer>,
    mut commands: Commands,
) -> Result {
    let point_cloud_handle = point_cloud_server.load::<LasLoader<_>>(FileSource::open(
        "assets/pointclouds/lion_takanawa.copc.laz",
    )?);
    commands.spawn((
        // PointCloud3d(
        //     point_cloud_server.load::<PointCloudMeshLoader>(Sphere::default().mesh().uv(32,
        // 18)), ),
        PointCloud3d(point_cloud_handle),
        Transform::from_rotation(Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2)),
        PointCloudMaterial3d(materials.add(SimplePointCloudMaterial {
            base_color: Color::srgb(0.0, 0.2, 0.4), // Deep Blue
            color_stops: vec![
                ColorStop {
                    color: Color::srgb(0.0, 0.5, 0.7),
                    point: 0.15,
                }, // Teal / Shallow water
                ColorStop {
                    color: Color::srgb(0.2, 0.6, 0.3),
                    point: 0.30,
                }, // Lush Green / Lowlands
                ColorStop {
                    color: Color::srgb(0.4, 0.7, 0.3),
                    point: 0.45,
                }, // Yellow-Green / Hills
                ColorStop {
                    color: Color::srgb(0.8, 0.8, 0.4),
                    point: 0.60,
                }, // Sand / Plateau
                ColorStop {
                    color: Color::srgb(0.6, 0.5, 0.3),
                    point: 0.72,
                }, // Light Brown / Mountain base
                ColorStop {
                    color: Color::srgb(0.4, 0.3, 0.2),
                    point: 0.82,
                }, // Dark Brown / High rock
                ColorStop {
                    color: Color::srgb(0.5, 0.5, 0.5),
                    point: 0.90,
                }, // Grey / Sub-alpine
                ColorStop {
                    color: Color::srgb(0.8, 0.8, 0.8),
                    point: 0.95,
                }, // Light Grey / Ice limits
            ],
            end_color: Color::srgb(1.0, 1.0, 1.0).into(), // Snow White
            gradient_direction: Some(Vec3::new(0.0, 1.0, 0.0)),
            gradient_start: Some(-0.5),
            gradient_end: Some(0.5),
            point_size: 0.025,
            ..default()
        })),
    ));

    Ok(())
}

fn update_material_gradient(
    added_aabbs: Query<(&Aabb, &GlobalTransform), (With<PointCloud3d>, Added<Aabb>)>,
    mut materials: ResMut<Assets<SimplePointCloudMaterial>>,
    material: Query<&PointCloudMaterial3d<SimplePointCloudMaterial>>,
) {
    let material_handle = material.single().unwrap();

    let Some(material) = materials.get(material_handle).cloned() else {
        return;
    };

    for (aabb, global_transform) in added_aabbs {
        let world_from_local = global_transform.affine();
        let min = world_from_local.transform_point3a(aabb.min());
        let max = world_from_local.transform_point3a(aabb.max());

        if material.gradient_start.unwrap() > min.y || material.gradient_end.unwrap() > max.y {
            // we get mut ref only if needed to prevent triggerring detection change if not needed
            let mut material_mut = materials.get_mut(material_handle).unwrap();
            material_mut.gradient_start = Some(min.y);
            material_mut.gradient_end = Some(max.y);
        }
    }
}
