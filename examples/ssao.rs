#![expect(missing_docs, reason = "Not all docs are written yet.")]

use std::f32::consts::PI;

use bevy::{
    camera::primitives::Aabb,
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    color::palettes::css::{GREEN, RED},
    core_pipeline::prepass::{DepthPrepass, NormalPrepass},
    dev_tools::{
        fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig},
        render_debug::{RenderDebugMode, RenderDebugOverlay, RenderDebugOverlayPlugin},
    },
    light::CascadeShadowConfigBuilder,
    pbr::ScreenSpaceAmbientOcclusion,
    platform::collections::HashSet,
    prelude::*,
};
use bevy_pointcloud::{
    prelude::*, ClassificationExt, CopcLoader, FilterClassification, PointCloudMeshLoader,
    SplatOrientation, SplatSettings, StandardPointCloudMaterial,
};
use las::point::Classification;

// --- MAIN FUNCTION ---

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FpsOverlayPlugin {
            config: FpsOverlayConfig {
                text_config: TextFont {
                    font_size: FontSize::Px(42.0),
                    font: default(),
                    font_smoothing: FontSmoothing::default(),
                    ..default()
                },
                text_color: GREEN.into(),
                refresh_interval: core::time::Duration::from_millis(100),
                enabled: true,
                frame_time_graph_config: FrameTimeGraphConfig {
                    enabled: true,
                    min_fps: 30.0,
                    target_fps: 144.0,
                },
            },
        })
        .add_plugins(FreeCameraPlugin)
        .add_plugins(PointCloudPlugin::default())
        // Startup Systems
        .add_systems(Startup, (setup, load_sphere_point_cloud))
        .add_systems(PostUpdate, center_point_cloud)
        .insert_resource(GlobalAmbientLight {
            brightness: 1000.,
            ..default()
        })
        .run();
}

// --- STARTUP SYSTEMS ---

fn setup(mut commands: Commands) {
    // Basic 3D camera setup with SSAO & Post-Processing
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(3.0, 2.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
        FreeCamera::default(),
        PointCloudVisibilitySettings {
            min_radius: Some(30.0),
            point_budget: Some(10_000_000),
            ..default()
        },
        Msaa::Off,
        // DepthPrepass,
        // NormalPrepass,
        ScreenSpaceAmbientOcclusion {
            quality_level: bevy::pbr::ScreenSpaceAmbientOcclusionQualityLevel::High,
            constant_object_thickness: 1000.0,
        },
        // RenderDebugOverlay {
        //     enabled: true,
        //     mode: RenderDebugMode::Normal,
        //     opacity: 1.0,
        // },
    ));

    // Directional Light with Shadows to emphasize SSAO and Depth
    commands.spawn((
        DirectionalLight {
            illuminance: light_consts::lux::OVERCAST_DAY,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform {
            translation: Vec3::new(0.0, 10.0, 0.0),
            rotation: Quat::from_rotation_x(-PI / 4.0),
            ..default()
        },
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 1.0,
            maximum_distance: 1000.0,
            ..default()
        }
        .build(),
    ));
}

fn load_sphere_point_cloud(
    mut meshes: ResMut<Assets<Mesh>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    mut pc_standard_materials: ResMut<Assets<StandardPointCloudMaterial>>,
    mut pc_simple_materials: ResMut<Assets<SimplePointCloudMaterial>>,
    point_cloud_server: Res<PointCloudServer>,
    mut commands: Commands,
) -> Result<()> {
    // 1. Generate a procedural sphere mesh and load it through PointCloudServer
    let point_cloud_handle =
        point_cloud_server.load::<PointCloudMeshLoader>(Sphere::new(1.0).mesh().ico(8)?);

    let pbr_material_handle = standard_materials.add(StandardMaterial {
        base_color: RED.into(),
        perceptual_roughness: 0.8,
        reflectance: 0.2,
        ..default()
    });

    // 2. Create the Point Cloud Material
    let pbr_pc_material_handle =
        pc_standard_materials.add(StandardPointCloudMaterial(StandardMaterial {
            base_color: RED.into(),
            cull_mode: None,
            perceptual_roughness: 0.8,
            reflectance: 0.2,
            ..default()
        }));

    let pc_material_handle = pc_simple_materials.add(SimplePointCloudMaterial {
        base_color: RED.into(),
        cull_mode: None,
        // perceptual_roughness: 0.8,
        // reflectance: 0.2,
        ..default()
    });

    // 3. Spawn entity with Cubes as Splats
    commands.spawn((
        PointCloud3d(point_cloud_handle),
        PointCloudMaterial3d(pbr_pc_material_handle),
        // PointCloudMaterial3d(pc_material_handle),
        SplatSettings {
            // Assign a Cuboid mesh as the individual splat geometry
            splat: Some(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
            // radius: Some(0.5),
            point_size_mode: PointSizeMode::LocalSpace,
            point_size: 0.05,
            orientation: SplatOrientation::FaceNormal,
            default_normal: Vec3::Z,
            ..default()
        },
    ));

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(pbr_material_handle),
        Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
    ));

    let point_cloud_vegetation_handle = point_cloud_server.load_with_settings::<CopcLoader<_>>(FileSource::open(
        "/home/romain/Documents/PointClouds/LidarHD/LHD_FXX_0893_6238_PTS_LAMB93_IGN69.copc.laz",
    )?, |settings| {
        settings.filter_classification = FilterClassification::Include(HashSet::from([
            Classification::HighVegetation.as_u8(),
            Classification::LowVegetation.as_u8(),
        ]));
    });

    let point_cloud_other_handle = point_cloud_server.load_with_settings::<CopcLoader<_>>(FileSource::open(
        "/home/romain/Documents/PointClouds/LidarHD/LHD_FXX_0893_6238_PTS_LAMB93_IGN69.copc.laz",
    )?, |settings| {
        settings.filter_classification = FilterClassification::Exclude(HashSet::from([
            Classification::HighVegetation.as_u8(),
            Classification::LowVegetation.as_u8(),
        ]));
    });

    commands.spawn((
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        Visibility::default(),
        children![
            (
                CopcPointCloud,
                PointCloud3d(point_cloud_other_handle),
                // PointCloudMaterial3d(
                //     pc_standard_materials.add(StandardPointCloudMaterial(Color::from(RED).
                // into())) ),
                PointCloudMaterial3d(pc_standard_materials.add(StandardPointCloudMaterial(
                    StandardMaterial {
                        base_color: RED.into(),
                        // cull_mode: None,
                        perceptual_roughness: 0.8,
                        reflectance: 0.2,
                        ..default()
                    }
                ))),
                // PointCloudMaterial3d(pc_simple_materials.add(Color::from(RED))),
                SplatSettings {
                    // splat: Some(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
                    point_size_mode: PointSizeMode::LocalSpace,
                    point_size: 0.5,
                    radius: Some(0.5),
                    orientation: SplatOrientation::Billboard,
                    // orientation: SplatOrientation::FaceNormal,
                    // default_normal: Vec3::new(0.0, 0.0, 1.0),
                    // uv_mapping: UVMapping::Planar,
                    // uv_u: Vec3::new(0.0, 1.0, 0.0),
                    // uv_v: Vec3::new(1.0, 0.0, 0.0),
                    // uv_transform: Affine2::from_scale_angle_translation(
                    //     Vec2 { x: 10.0, y: 10.0 },
                    //     0.0,
                    //     Vec2::ZERO,
                    // ),
                    ..default()
                }
            ),
            (
                CopcPointCloud,
                PointCloud3d(point_cloud_vegetation_handle),
                // PointCloudMaterial3d(
                //     pc_standard_materials
                //         .add(StandardPointCloudMaterial(Color::from(GREEN).into()))
                // ),
                PointCloudMaterial3d(pc_standard_materials.add(StandardPointCloudMaterial(
                    StandardMaterial {
                        base_color: GREEN.into(),
                        // cull_mode: None,
                        perceptual_roughness: 0.8,
                        reflectance: 0.2,
                        ..default()
                    }
                ))),
                SplatSettings {
                    // splat: Some(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
                    point_size_mode: PointSizeMode::LocalSpace,
                    point_size: 0.5,
                    radius: Some(0.5),
                    orientation: SplatOrientation::Billboard,
                    // orientation: SplatOrientation::FaceNormal,
                    // default_normal: Vec3::new(0.0, 0.0, 1.0),
                    // uv_mapping: UVMapping::Planar,
                    // uv_u: Vec3::new(0.0, 1.0, 0.0),
                    // uv_v: Vec3::new(1.0, 0.0, 0.0),
                    // uv_transform: Affine2::from_scale_angle_translation(
                    //     Vec2 { x: 10.0, y: 10.0 },
                    //     0.0,
                    //     Vec2::ZERO,
                    // ),
                    ..default()
                }
            )
        ],
    ));

    Ok(())
}

#[derive(Component)]
struct CopcPointCloud;

// --- POST-UPDATE SYSTEMS ---

fn center_point_cloud(
    loaded_point_clouds: Query<
        (Entity, &Aabb),
        (With<PointCloud3d>, With<CopcPointCloud>, Added<Aabb>),
    >,
    mut commands: Commands,
) {
    for (entity, aabb) in loaded_point_clouds {
        let center = aabb.center;
        commands
            .entity(entity)
            .insert(Transform::from_translation(Vec3 {
                x: -center.x,
                y: -center.y,
                z: 0.0, // Keep the original altitude
            }));
    }
}
