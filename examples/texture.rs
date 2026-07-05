#![expect(missing_docs, reason = "Not all docs are written yet.")]

mod utils;

use std::f32::consts::TAU;

use bevy::{
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::Face,
    transform::systems::propagate_parent_transforms,
};
use bevy_pointcloud::{las::LasLoader, prelude::*, ShapeOrientation, UVTransform};

use crate::utils::draw_gizmos;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FreeCameraPlugin)
        .add_plugins(PointCloudPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(Startup, load_point_cloud)
        .add_systems(Update, update_material)
        .add_systems(PostUpdate, draw_gizmos.after(propagate_parent_transforms))
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

    let sphere_mesh = Sphere::new(0.5).mesh().ico(4)?;
    // sphere_mesh.generate_tangents()?;

    let point_cloud_sphere = point_cloud_server.load::<PointCloudMeshLoader>(sphere_mesh);

    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        PointCloud3d(point_cloud_sphere.clone()),
        PointCloudMaterial3d(point_cloud_materials.add(SimplePointCloudMaterial {
            shape_radius: Some(0.5),
            point_size_mode: PointSizeMode::ScreenPixelsLocal,
            point_size: 80.0,
            shape_orientation: ShapeOrientation::FaceNormal,
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

    commands.spawn((
        PointCloud3d(point_cloud_sphere.clone()),
        PointCloudMaterial3d(point_cloud_materials.add(SimplePointCloudMaterial {
            shape_radius: Some(0.5),
            point_size_mode: PointSizeMode::LocalSpace,
            point_size: 0.1,
            shape_orientation: ShapeOrientation::Billboard,
            base_color_texture: Some(texture_handle.clone()),
            uv_mapping: bevy_pointcloud::UVMapping::Planar,
            uv_u: Vec3::new(0.0, 0.0, -1.0),
            uv_v: Vec3::new(0.0, -1.0, 0.0),
            uv_transform: Some(UVTransform {
                offset: Vec2 { x: -0.5, y: -0.5 },
                scale: Vec2 { x: 2.0, y: 2.0 },
                rotation: 0.0,
            }),
            ..Default::default()
        })),
        Transform::from_translation(Vec3::new(0.0, 5.0, -5.0)).with_scale(Vec3::splat(5.0)),
    ));

    commands.spawn((
        PointCloud3d(point_cloud_sphere.clone()),
        PointCloudMaterial3d(point_cloud_materials.add(SimplePointCloudMaterial {
            shape_radius: Some(0.5),
            point_size_mode: PointSizeMode::LocalSpace,
            point_size: 0.1,
            shape_orientation: ShapeOrientation::FaceNormal,
            base_color_texture: Some(texture_handle.clone()),
            uv_mapping: bevy_pointcloud::UVMapping::Planar,
            uv_u: Vec3::new(0.0, 0.0, -1.0),
            uv_v: Vec3::new(0.0, -1.0, 0.0),
            uv_transform: Some(UVTransform {
                offset: Vec2 { x: -0.5, y: -0.5 },
                scale: Vec2 { x: 2.0, y: 2.0 },
                rotation: 0.0,
            }),
            ..Default::default()
        })),
        Transform::from_translation(Vec3::new(0.0, 5.0, 0.0)).with_scale(Vec3::splat(5.0)),
    ));

    commands.spawn((
        PointCloud3d(point_cloud_sphere.clone()),
        PointCloudMaterial3d(point_cloud_materials.add(SimplePointCloudMaterial {
            shape_radius: Some(0.5),
            point_size_mode: PointSizeMode::LocalSpace,
            point_size: 0.1,
            shape_orientation: ShapeOrientation::FaceNormal,
            base_color_texture: Some(texture_handle.clone()),
            uv_mapping: bevy_pointcloud::UVMapping::Combined,
            uv_transform: Some(UVTransform {
                offset: Vec2 { x: -0.5, y: -0.5 },
                scale: Vec2 { x: 2.0, y: 2.0 },
                rotation: 0.0,
            }),
            ..Default::default()
        })),
        Transform::from_translation(Vec3::new(0.0, 5.0, 5.0)).with_scale(Vec3::splat(5.0)),
    ));

    let plane_3d = Plane3d::new(Vec3::Z, Vec2::new(1.0, 1.0))
        .mesh()
        .subdivisions(49)
        .build();
    // plane_3d.generate_tangents()?;

    let animated_material = point_cloud_materials.add(SimplePointCloudMaterial {
        shape_radius: Some(0.5),
        point_size_mode: PointSizeMode::LocalSpace,
        point_size: 0.04,
        shape_orientation: ShapeOrientation::FaceNormal,
        base_color_texture: Some(texture_handle.clone()),
        uv_mapping: bevy_pointcloud::UVMapping::Combined,
        cull_mode: None,
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

    let animated_material_combined = point_cloud_materials.add(SimplePointCloudMaterial {
        shape_radius: Some(0.5),
        point_size_mode: PointSizeMode::LocalSpace,
        point_size: 0.04,
        shape_orientation: ShapeOrientation::Billboard,
        base_color_texture: Some(texture_handle.clone()),
        uv_mapping: bevy_pointcloud::UVMapping::Combined,
        cull_mode: Some(Face::Back),
        uv_u: Vec3::new(1.0, 0.0, 0.0),
        uv_v: Vec3::new(0.0, 1.0, 0.0),
        uv_transform: Some(UVTransform {
            offset: Vec2 { x: -0.5, y: -0.5 },
            scale: Vec2 { x: 2.0, y: 2.0 },
            rotation: 0.0,
        }),
        ..Default::default()
    });

    commands.spawn((
        MyPlan,
        PointCloud3d(point_cloud_plan.clone()),
        PointCloudMaterial3d(animated_material_combined.clone()),
        Transform::from_translation(Vec3::new(0.0, -2.0, 4.0))
            .with_rotation(Quat::from_axis_angle(Vec3::Y, std::f32::consts::FRAC_PI_2)),
    ));

    let animated_material_combined_screen_space_point_size_face_normal =
        point_cloud_materials.add(SimplePointCloudMaterial {
            shape_radius: Some(0.5),
            point_size_mode: PointSizeMode::ScreenPixelsLocal,
            point_size: 30.0,
            shape_orientation: ShapeOrientation::FaceNormal,
            base_color_texture: Some(texture_handle.clone()),
            uv_mapping: bevy_pointcloud::UVMapping::Combined,
            uv_u: Vec3::new(1.0, 0.0, 0.0),
            uv_v: Vec3::new(0.0, 1.0, 0.0),
            uv_transform: Some(UVTransform {
                offset: Vec2 { x: -0.5, y: -0.5 },
                scale: Vec2 { x: 2.0, y: 2.0 },
                rotation: 0.0,
            }),
            ..Default::default()
        });

    commands.spawn((
        MyPlan,
        PointCloud3d(point_cloud_plan.clone()),
        PointCloudMaterial3d(
            animated_material_combined_screen_space_point_size_face_normal.clone(),
        ),
        Transform::from_translation(Vec3::new(0.0, 0.0, 6.0))
            .with_rotation(Quat::from_axis_angle(Vec3::Y, std::f32::consts::FRAC_PI_2)),
    ));

    let animated_material_combined_screen_space_point_size_billboard =
        point_cloud_materials.add(SimplePointCloudMaterial {
            shape_radius: Some(0.5),
            point_size_mode: PointSizeMode::ScreenPixelsLocal,
            point_size: 30.0,
            shape_orientation: ShapeOrientation::Billboard,
            base_color_texture: Some(texture_handle.clone()),
            uv_mapping: bevy_pointcloud::UVMapping::Combined,
            cull_mode: Some(Face::Back),
            uv_u: Vec3::new(1.0, 0.0, 0.0),
            uv_v: Vec3::new(0.0, 1.0, 0.0),
            uv_transform: Some(UVTransform {
                offset: Vec2 { x: -0.5, y: -0.5 },
                scale: Vec2 { x: 2.0, y: 2.0 },
                rotation: 0.0,
            }),
            ..Default::default()
        });

    commands.spawn((
        MyPlan,
        PointCloud3d(point_cloud_plan.clone()),
        PointCloudMaterial3d(animated_material_combined_screen_space_point_size_billboard.clone()),
        Transform::from_translation(Vec3::new(0.0, -2.0, 6.0))
            .with_rotation(Quat::from_axis_angle(Vec3::Y, std::f32::consts::FRAC_PI_2)),
    ));

    let animated_material_planar_screen_space_point_size_face_normal =
        point_cloud_materials.add(SimplePointCloudMaterial {
            shape_radius: Some(0.5),
            point_size_mode: PointSizeMode::ScreenPixelsLocal,
            point_size: 30.0,
            shape_orientation: ShapeOrientation::FaceNormal,
            base_color_texture: Some(texture_handle.clone()),
            uv_mapping: bevy_pointcloud::UVMapping::Planar,
            cull_mode: None,
            uv_u: Vec3::new(1.0, 0.0, 0.0),
            uv_v: Vec3::new(0.0, 1.0, 0.0),
            uv_transform: Some(UVTransform {
                offset: Vec2 { x: -0.5, y: -0.5 },
                scale: Vec2 { x: 2.0, y: 2.0 },
                rotation: 0.0,
            }),
            ..Default::default()
        });

    commands.spawn((
        MyPlan,
        PointCloud3d(point_cloud_plan.clone()),
        PointCloudMaterial3d(animated_material_planar_screen_space_point_size_face_normal.clone()),
        Transform::from_translation(Vec3::new(0.0, 0.0, 8.0))
            .with_rotation(Quat::from_axis_angle(Vec3::Y, std::f32::consts::FRAC_PI_2)),
    ));

    let animated_material_planar_screen_space_point_size_billboard =
        point_cloud_materials.add(SimplePointCloudMaterial {
            shape_radius: Some(0.5),
            point_size_mode: PointSizeMode::ScreenPixelsLocal,
            point_size: 30.0,
            shape_orientation: ShapeOrientation::Billboard,
            base_color_texture: Some(texture_handle.clone()),
            uv_mapping: bevy_pointcloud::UVMapping::Planar,
            uv_u: Vec3::new(1.0, 0.0, 0.0),
            uv_v: Vec3::new(0.0, 1.0, 0.0),
            uv_transform: Some(UVTransform {
                offset: Vec2 { x: -0.5, y: -0.5 },
                scale: Vec2 { x: 2.0, y: 2.0 },
                rotation: 0.0,
            }),
            ..Default::default()
        });

    commands.spawn((
        MyPlan,
        PointCloud3d(point_cloud_plan.clone()),
        PointCloudMaterial3d(animated_material_planar_screen_space_point_size_billboard.clone()),
        Transform::from_translation(Vec3::new(0.0, -2.0, 8.0))
            .with_rotation(Quat::from_axis_angle(Vec3::Y, std::f32::consts::FRAC_PI_2)),
    ));

    let lion_animated_material = point_cloud_materials.add(SimplePointCloudMaterial {
        shape_radius: Some(0.5),
        point_size_mode: PointSizeMode::LocalSpace,
        point_size: 0.04,
        shape_orientation: ShapeOrientation::Billboard,
        base_color_texture: Some(texture_handle.clone()),
        uv_mapping: bevy_pointcloud::UVMapping::Planar,
        cull_mode: Some(Face::Back),
        uv_u: Vec3::new(1.0, 0.0, 0.0),
        uv_v: Vec3::new(0.0, 1.0, 0.0),
        uv_transform: Some(UVTransform {
            offset: Vec2 { x: -0.5, y: -0.5 },
            scale: Vec2 { x: 2.0, y: 2.0 },
            rotation: 0.0,
        }),
        ..Default::default()
    });

    let point_cloud_handle = point_cloud_server.load::<LasLoader<_>>(FileSource::open(
        "assets/pointclouds/lion_takanawa.copc.laz",
    )?);
    commands.spawn((
        MyPlan,
        PointCloud3d(point_cloud_handle.clone()),
        PointCloudMaterial3d(lion_animated_material.clone()),
        Transform::from_rotation(Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2))
            .with_translation(Vec3::new(0.0, 0.0, -2.0)),
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

            // material.uv_transform = None;

            if let Some(uv_transform) = &mut material.uv_transform {
                uv_transform.rotation = modulo;
                uv_transform.scale =
                    Vec2::new(1.0 + modulo.sin() / 2.0, 1.0 + modulo_2.cos() / 2.0);
            }
        }
    }
}
