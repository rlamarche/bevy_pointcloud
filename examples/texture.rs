#![expect(missing_docs, reason = "Not all docs are written yet.")]

use std::f32::consts::TAU;

use bevy::{
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::Face,
};
use bevy_pointcloud::{las::LasLoader, prelude::*, UVTransform};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FreeCameraPlugin)
        .add_plugins(PointCloudPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(Startup, load_point_cloud)
        .add_systems(Update, update_material)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(2.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        FreeCamera::default(),
    ));
}

#[derive(Component)]
struct MyPlan;

fn load_point_cloud(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut point_cloud_materials: ResMut<Assets<SimplePointCloudMaterial>>,
    point_cloud_server: Res<PointCloudServer>,
) -> Result {
    let texture_handle = asset_server
        .load_builder()
        .with_settings(|settings: &mut ImageLoaderSettings| {
            settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                ..default()
            });
        })
        .load("branding/bevy_icon.png");

    let point_cloud =
        point_cloud_server.load::<PointCloudMeshLoader>(Sphere::new(0.5).mesh().ico(4)?);

    // commands.spawn((
    //     PointCloud3d(point_cloud.clone()),
    //     PointCloudMaterial3d(point_cloud_materials.add(SimplePointCloudMaterial {
    //         // shape_radius: Some(0.5),
    //         // shape_mesh: Some(meshes.add(Sphere::new(0.5).mesh().ico(16)?)),
    //         point_size: 0.1,
    //         shape_orientation: bevy_pointcloud::ShapeOrientation::FaceNormal,
    //         base_color_texture: Some(texture_handle.clone()),
    //         uv_mapping: bevy_pointcloud::UVMapping::Combined,
    //         ..Default::default()
    //     })),
    //     Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)).with_scale(Vec3::splat(5.0)),
    // ));

    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    // commands.spawn((
    //     PointCloud3d(point_cloud.clone()),
    //     PointCloudMaterial3d(point_cloud_materials.add(SimplePointCloudMaterial {
    //         shape_radius: Some(0.5),
    //         point_size: 0.1,
    //         shape_orientation: bevy_pointcloud::ShapeOrientation::FaceNormal,
    //         base_color_texture: Some(texture_handle.clone()),
    //         uv_mapping: bevy_pointcloud::UVMapping::PointCloudOnly,
    //         ..Default::default()
    //     })),
    //     Transform::from_translation(Vec3::new(-5.0, 0.0, 1.0)).with_scale(Vec3::splat(5.0)),
    // ));
    commands.spawn((
        PointCloud3d(point_cloud.clone()),
        PointCloudMaterial3d(point_cloud_materials.add(SimplePointCloudMaterial {
            shape_radius: Some(0.5),
            point_size: 0.1,
            shape_orientation: bevy_pointcloud::ShapeOrientation::FaceNormal,
            base_color_texture: Some(texture_handle.clone()),
            uv_mapping: bevy_pointcloud::UVMapping::ShapeOnly,
            uv_transform: Some(UVTransform {
                offset: Vec2 { x: -0.5, y: -0.5 },
                scale: Vec2 { x: 2.0, y: 2.0 },
                rotation: 0.0,
            }),
            ..Default::default()
        })),
        Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)).with_scale(Vec3::splat(5.0)),
    ));
    // commands.spawn((
    //     PointCloud3d(point_cloud.clone()),
    //     PointCloudMaterial3d(point_cloud_materials.add(SimplePointCloudMaterial {
    //         shape_radius: Some(0.5),
    //         point_size: 0.1,
    //         shape_orientation: bevy_pointcloud::ShapeOrientation::FaceNormal,
    //         base_color_texture: Some(texture_handle.clone()),
    //         uv_mapping: bevy_pointcloud::UVMapping::Planar,
    //         ..Default::default()
    //     })),
    //     Transform::from_translation(Vec3::new(0.0, 0.0, 2.0)).with_scale(Vec3::splat(5.0)),
    // ));

    let mut plane_3d = Plane3d::new(Vec3::Z, Vec2::new(1.0, 1.0))
        .mesh()
        .subdivisions(49)
        .build();
    plane_3d.generate_tangents()?;

    let animated_material = point_cloud_materials.add(SimplePointCloudMaterial {
        shape_radius: Some(0.5),
        point_size: 0.04,
        shape_orientation: bevy_pointcloud::ShapeOrientation::Billboard,
        base_color_texture: Some(texture_handle.clone()),
        uv_mapping: bevy_pointcloud::UVMapping::Planar,
        cull_mode: Some(Face::Back),
        uv_u: Vec3::new(1.0, 0.0, 0.0),
        uv_v: Vec3::new(0.0, -1.0, 0.0),
        uv_transform: Some(UVTransform {
            offset: Vec2 { x: -0.5, y: -0.5 },
            scale: Vec2 { x: 2.0, y: 2.0 },
            rotation: 0.0,
        }),
        ..Default::default()
    });

    let point_cloud_plan = point_cloud_server.load::<PointCloudMeshLoader>(plane_3d);
    commands.spawn((
        MyPlan,
        PointCloud3d(point_cloud_plan.clone()),
        PointCloudMaterial3d(animated_material.clone()),
        Transform::from_translation(Vec3::new(0.0, 0.0, 4.0))
            .with_rotation(Quat::from_axis_angle(Vec3::Y, std::f32::consts::FRAC_PI_2)),
    ));

    let point_cloud_handle = point_cloud_server.load::<LasLoader<_>>(FileSource::open(
        "assets/pointclouds/lion_takanawa.copc.laz",
    )?);
    commands.spawn((
        MyPlan,
        PointCloud3d(point_cloud_handle.clone()),
        PointCloudMaterial3d(animated_material.clone()),
        Transform::from_rotation(Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2)),
    ));

    Ok(())
}

fn update_material(
    mut materials: ResMut<Assets<SimplePointCloudMaterial>>,
    material: Query<&PointCloudMaterial3d<SimplePointCloudMaterial>>,
    time: Res<Time<Real>>,
) {
    for material in material.iter() {
        if let Some(mut material) = materials.get_mut(material) {
            let modulo = (time.elapsed().as_millis() % 20000) as f32 * TAU / 20000.0;
            let modulo_2 = (time.elapsed().as_millis() % 40000) as f32 * TAU / 40000.0;

            if let Some(uv_transform) = &mut material.uv_transform {
                uv_transform.rotation = modulo;
                uv_transform.scale =
                    Vec2::new(1.0 + modulo.sin() / 2.0, 1.0 + modulo_2.cos() / 2.0);
            }
        }
    }
}
