#![expect(missing_docs, reason = "Not all docs are written yet.")]
use std::{f32::consts::PI, ops::Mul};

use bevy::{
    camera::primitives::Aabb,
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    color::palettes::css::RED,
    feathers::FeathersPlugins,
    image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    math::Affine2,
    prelude::*,
    transform::systems::propagate_parent_transforms,
};
use bevy_pointcloud::{prelude::*, ColorStop, CopcLoader, VisiblePointCloudOctreeEntities};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FeathersPlugins)
        .add_plugins(FreeCameraPlugin)
        // Initializes the core rendering architecture for point clouds
        .add_plugins(PointCloudPlugin::default())
        // .add_plugins(MyUiPlugin)
        // .insert_resource(UiTheme(create_dark_theme()))
        .add_systems(Startup, (setup, load_point_cloud))
        .add_systems(PostUpdate, setup_point_cloud)
        .add_systems(PreUpdate, draw_gizmos.after(propagate_parent_transforms))
        .run();
}

fn setup(mut commands: Commands) {
    // Basic 3D camera setup looking towards the origin
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(1.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        FreeCamera::default(),
        PointCloudVisibilitySettings {
            max_depth: Some(3),
            ..default()
        },
    ));
}

fn load_point_cloud(
    mut materials: ResMut<Assets<SimplePointCloudMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
    point_cloud_server: Res<PointCloudServer>,
    mut commands: Commands,
) -> Result<()> {
    let point_cloud_handle = point_cloud_server.load::<CopcLoader<_>>(FileSource::open(
        // "assets/pointclouds/lion_takanawa.copc.laz",
        "/home/romain/Documents/PointClouds/LidarHD/LHD_FXX_0893_6238_PTS_LAMB93_IGN69.copc.laz",
    )?);

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

    let material_handle = materials.add(SimplePointCloudMaterial {
        // min_point_size: Some(2.0),
        // max_point_size: Some(50.0),
        base_color: Color::srgb(0.0, 0.2, 0.4), // Deep Blue
        base_color_texture: Some(texture_handle.clone()),
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
        gradient_start: Some(0.0),
        gradient_end: Some(500.0),
        ..default()
    });

    commands.spawn((
        Transform::from_rotation(Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2)),
        children![(
            PointCloud3d(point_cloud_handle),
            PointCloudMaterial3d(material_handle),
            SplatSettings {
                splat: Some(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
                // radius: Some(0.5),
                point_size_mode: PointSizeMode::LocalSpace,
                point_size: 0.5,
                uv_mapping: UVMapping::Planar,
                uv_u: Vec3::new(1.0, 0.0, 0.0),
                uv_v: Vec3::new(0.0, 1.0, 0.0),
                uv_transform: Affine2::from_scale_angle_translation(
                    Vec2 { x: 10.0, y: 10.0 },
                    PI / 2.0,
                    Vec2::ZERO,
                ),
                // uv_transform: Some(UVTransform {
                //     // offset: Vec2 { x: -0.5, y: -0.5 },
                //     scale: Vec2 { x: 10.0, y: 10.0 },
                //     rotation: PI / 2.0,
                //     ..default()
                // }),
                ..default()
            }
        )],
    ));

    Ok(())
}

fn setup_point_cloud(
    loaded_point_clouds: Query<
        (
            Entity,
            &Aabb,
            &PointCloudMaterial3d<SimplePointCloudMaterial>,
        ),
        (With<PointCloud3d>, Added<Aabb>),
    >,
    mut _materials: ResMut<Assets<SimplePointCloudMaterial>>,
    mut commands: Commands,
) {
    for (entity, aabb, _material) in loaded_point_clouds {
        let center = aabb.center;
        commands
            .entity(entity)
            .insert(Transform::from_translation(Vec3 {
                x: -center.x,
                y: -center.y,
                z: 0.0, // to keep the altitude
            }));

        dbg!(aabb);
        // // update gradient bounds
        // if let Some(mut material) = materials.get_mut(material) {
        //     material.gradient_start = Some(aabb.min().z);
        //     material.gradient_end = Some(aabb.max().z);
        // }
    }
}

fn draw_gizmos(
    point_clouds: Res<Assets<PointCloud>>,
    entities: Query<&GlobalTransform, With<PointCloud3d>>,
    visible_point_cloud_entities: Query<&VisiblePointCloudOctreeEntities>,
    mut gizmos: Gizmos,
) {
    // for each view
    for visible_point_cloud_entities in visible_point_cloud_entities {
        // for each visible point cloud in this view
        for (entity, visible_point_cloud_entity) in &visible_point_cloud_entities.entities {
            let Ok(global_transform) = entities.get(*entity) else {
                continue;
            };

            let Some(point_cloud) = point_clouds.get(visible_point_cloud_entity.asset_id) else {
                continue;
            };

            // for each visible node in this view
            for visible_node in &visible_point_cloud_entity.node_entities {
                // we only draw octree's gizmos
                let Some(octree) = point_cloud.topology.as_octree() else {
                    continue;
                };

                let Some(node) = octree.get_node(visible_node.id) else {
                    continue;
                };

                if let Some(aabb) = &node.aabb {
                    let center = aabb.center;
                    let scale = aabb.half_extents.mul(2.0);

                    let local_transform =
                        Transform::from_translation(center.into()).with_scale(scale.into());

                    let world_transform = global_transform.mul_transform(local_transform);

                    gizmos.cube(world_transform, RED);
                }
            }
        }
    }
}
