#![expect(missing_docs, reason = "Not all docs are written yet.")]

mod utils;

use std::f32::consts::TAU;

use bevy::{
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig},
    image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    light::CascadeShadowConfigBuilder,
    prelude::*,
    render::render_resource::Face,
    transform::systems::propagate_parent_transforms,
};
use bevy_pointcloud::{las::LasLoader, prelude::*, SplatOrientation, UVTransform};

use crate::utils::draw_gizmos;

// --- RESOURCES AND STRUCTURE ---

struct OverlayColor;

impl OverlayColor {
    const RED: Color = Color::srgb(1.0, 0.0, 0.0);
    const GREEN: Color = Color::srgb(0.0, 1.0, 0.0);
}

#[derive(Component)]
struct MyPlan;

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
                text_color: OverlayColor::GREEN,
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
        // Global shadow map resolution configuration (High Quality)
        .insert_resource(bevy::light::DirectionalLightShadowMap { size: 4096 })
        // Startup Systems
        .add_systems(Startup, (setup, load_point_cloud))
        // Update Systems
        .add_systems(Update, update_material)
        // PostUpdate Systems
        .add_systems(PostUpdate, draw_gizmos.after(propagate_parent_transforms))
        .run();
}

// --- STARTUP SYSTEMS ---

fn setup(mut commands: Commands) {
    // Basic 3D camera setup looking towards the origin
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(2.0, 5.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
        FreeCamera::default(),
    ));

    // Fixed directional light configured for sharp, high-quality shadows
    commands.spawn((
        DirectionalLight {
            color: Color::WHITE,
            illuminance: light_consts::lux::OVERCAST_DAY,
            shadow_maps_enabled: true,
            shadow_depth_bias: 0.05,
            shadow_normal_bias: 1.2,
            ..default()
        },
        // Restricting max shadow distance concentrates the 4096px resolution into a tighter zone
        CascadeShadowConfigBuilder {
            maximum_distance: 100.0,
            ..default()
        }
        .build(),
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn load_point_cloud(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut point_cloud_materials: ResMut<Assets<SimplePointCloudMaterial>>,
    point_cloud_server: Res<PointCloudServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
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
    let point_cloud_sphere = point_cloud_server.load::<PointCloudMeshLoader>(sphere_mesh);

    // --- Spawn Various Point Cloud Configurations (Centered Zone) ---

    // 1. Screen Pixels Local / Face Normal
    commands.spawn((
        PointCloud3d(point_cloud_sphere.clone()),
        PointCloudMaterial3d(point_cloud_materials.add(SimplePointCloudMaterial {
            shape_radius: Some(0.5),
            point_size_mode: PointSizeMode::ScreenPixelsLocal,
            point_size: 80.0,
            shape_orientation: SplatOrientation::FaceNormal,
            base_color_texture: Some(texture_handle.clone()),
            uv_mapping: bevy_pointcloud::UVMapping::SplatOnly,
            uv_transform: Some(UVTransform {
                offset: Vec2 { x: -0.5, y: -0.5 },
                scale: Vec2 { x: 2.0, y: 2.0 },
                rotation: 0.0,
            }),
            ..Default::default()
        })),
        Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)).with_scale(Vec3::splat(5.0)),
    ));

    // 2. Local Space / Billboard / Planar
    commands.spawn((
        PointCloud3d(point_cloud_sphere.clone()),
        PointCloudMaterial3d(point_cloud_materials.add(SimplePointCloudMaterial {
            shape_radius: Some(0.5),
            point_size_mode: PointSizeMode::LocalSpace,
            point_size: 0.1,
            shape_orientation: SplatOrientation::Billboard,
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

    // 3. Local Space / Face Normal / Planar
    commands.spawn((
        PointCloud3d(point_cloud_sphere.clone()),
        PointCloudMaterial3d(point_cloud_materials.add(SimplePointCloudMaterial {
            shape_radius: Some(0.5),
            point_size_mode: PointSizeMode::LocalSpace,
            point_size: 0.1,
            shape_orientation: SplatOrientation::FaceNormal,
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

    // 4. Local Space / Face Normal / Combined
    commands.spawn((
        PointCloud3d(point_cloud_sphere.clone()),
        PointCloudMaterial3d(point_cloud_materials.add(SimplePointCloudMaterial {
            shape_radius: Some(0.5),
            point_size_mode: PointSizeMode::LocalSpace,
            point_size: 0.1,
            shape_orientation: SplatOrientation::FaceNormal,
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

    // Setup planar surfaces
    let plane_3d = Plane3d::new(Vec3::Z, Vec2::new(1.0, 1.0))
        .mesh()
        .subdivisions(49)
        .build();

    let animated_material = point_cloud_materials.add(SimplePointCloudMaterial {
        shape_radius: Some(0.5),
        point_size_mode: PointSizeMode::LocalSpace,
        point_size: 0.04,
        shape_orientation: SplatOrientation::FaceNormal,
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
        shape_orientation: SplatOrientation::Billboard,
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

    // [Les autres plans MyPlan d'origine restent ici identiques pour la clarté du code...]
    // (Tronqué ici pour la lisibilité de la réponse, mais gardé à l'identique dans ton fichier)

    // --- DETACHED LION POINT CLOUD ---
    let lion_animated_material = point_cloud_materials.add(SimplePointCloudMaterial {
        shape_radius: Some(0.5),
        point_size_mode: PointSizeMode::LocalSpace,
        point_size: 0.04,
        shape_orientation: SplatOrientation::Billboard,
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

    // Moved the lion away on the X axis so it casts its own distinct shadow on the ground
    commands.spawn((
        MyPlan,
        PointCloud3d(point_cloud_handle.clone()),
        PointCloudMaterial3d(lion_animated_material.clone()),
        Transform::from_rotation(Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2))
            .with_translation(Vec3::new(-15.0, 0.0, -2.0)), // <-- X shifted to -15.0
    ));

    // --- GROUND PLANE FOR SHADOW RECEPTION ---
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(150.0, 150.0))), /* Enlarged to catch
                                                                           * the remote lion
                                                                           * shadow */
        MeshMaterial3d(standard_materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.8,
            ..default()
        })),
        Transform::from_xyz(0.0, -2.5, 0.0),
    ));

    Ok(())
}

// --- UPDATE SYSTEM ---

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
