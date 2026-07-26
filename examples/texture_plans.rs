#![expect(missing_docs, reason = "Not all docs are written yet.")]

use std::f32::consts::TAU;

use bevy::{
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    color::palettes::css::GREEN,
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig},
    image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    light::CascadeShadowConfigBuilder,
    math::Affine2,
    prelude::*,
    render::render_resource::Face,
};
use bevy_pointcloud::prelude::*;

// --- COMPONENTS ---

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
        // Global shadow map resolution configuration (High Quality)
        .insert_resource(bevy::light::DirectionalLightShadowMap { size: 4096 })
        // Startup Systems
        .add_systems(Startup, (setup, load_point_cloud))
        // Update Systems
        .add_systems(Update, update_material)
        .run();
}

// --- STARTUP SYSTEMS ---

fn setup(mut commands: Commands) {
    // Basic 3D camera setup looking towards the origin, positioned closer to see the planes clearly
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(2.0, 3.0, 8.0).looking_at(Vec3::new(0.0, -0.5, 0.0), Vec3::Y),
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
    mut point_cloud_materials: ResMut<Assets<StandardPointCloudMaterial>>,
    point_cloud_server: Res<PointCloudServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
) {
    // Load base texture with repeating address mode
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

    // Build the 3D plane mesh for point cloud loading
    let plane_mesh = Plane3d::new(Vec3::Z, Vec2::new(1.0, 1.0))
        .mesh()
        .subdivisions(49)
        .build();

    let point_cloud_plan = point_cloud_server.load::<PointCloudMeshLoader>(plane_mesh);

    // --- Create Materials ---

    // Animated material with no culling
    let animated_material = point_cloud_materials.add(StandardMaterial {
        base_color_texture: Some(texture_handle.clone()),
        cull_mode: None,
        ..Default::default()
    });

    // Animated material with back-face culling
    let animated_material_combined = point_cloud_materials.add(StandardMaterial {
        base_color_texture: Some(texture_handle.clone()),
        cull_mode: Some(Face::Back),
        ..Default::default()
    });

    // --- Spawn Planes ---

    let cuboid = meshes.add(Cuboid::new(1.0, 1.0, 1.0));

    // 1. Plane 1 (No Culling, FaceNormal Orientation, Combined UV mapping)
    commands.spawn((
        MyPlan,
        PointCloud3d(point_cloud_plan.clone()),
        PointCloudMaterial3d(animated_material.clone()),
        SplatSettings {
            splat: Some(cuboid.clone()),
            // radius: Some(0.5),
            point_size_mode: PointSizeMode::LocalSpace,
            point_size: 0.03,
            orientation: SplatOrientation::FaceNormal,
            uv_mapping: UVMapping::Planar,
            uv_transform: Affine2::from_scale_angle_translation(
                Vec2::splat(2.0),
                0.0,
                Vec2::new(-0.5, -0.5),
            ),
            uv_u: Vec3::new(0.0, 1.0, 0.0),
            uv_v: Vec3::new(1.0, 0.0, 0.0),
            ..default()
        },
        Transform::from_translation(Vec3::new(0.0, 1.0, 0.0))
            .with_rotation(Quat::from_axis_angle(Vec3::Y, std::f32::consts::FRAC_PI_2)),
    ));

    // 2. Plane 2 (Back-face Culling, Billboard Orientation, Combined UV mapping)
    commands.spawn((
        MyPlan,
        PointCloud3d(point_cloud_plan.clone()),
        PointCloudMaterial3d(animated_material_combined.clone()),
        SplatSettings {
            radius: Some(0.5),
            point_size_mode: PointSizeMode::LocalSpace,
            point_size: 0.04,
            orientation: SplatOrientation::Billboard,
            uv_mapping: UVMapping::Combined,
            uv_u: Vec3::new(1.0, 0.0, 0.0),
            uv_v: Vec3::new(0.0, 1.0, 0.0),
            uv_transform: Affine2::from_scale_angle_translation(
                Vec2::splat(2.0),
                0.0,
                Vec2::new(-0.5, -0.5),
            ),
            ..default()
        },
        Transform::from_translation(Vec3::new(0.0, -1.0, 0.0))
            .with_rotation(Quat::from_axis_angle(Vec3::Y, std::f32::consts::FRAC_PI_2)),
    ));

    // --- Ground Plane for Shadow Reception ---
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(150.0, 150.0))),
        MeshMaterial3d(standard_materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.8,
            ..default()
        })),
        Transform::from_xyz(0.0, -2.5, 0.0),
    ));
}

// --- UPDATE SYSTEM ---

fn update_material(
    mut splat_settings_query: Query<&mut SplatSettings, With<MyPlan>>,
    time: Res<Time<Real>>,
) {
    let modulo = (time.elapsed().as_millis() % 20000) as f32 * TAU / 20000.0;
    let modulo_2 = (time.elapsed().as_millis() % 40000) as f32 * TAU / 40000.0;

    let scale = Vec2::new(1.0 + modulo.sin() / 2.0, 1.0 + modulo_2.cos() / 2.0);
    let translation = Vec2::new(-0.5, -0.5);

    for mut splat_settings in splat_settings_query.iter_mut() {
        splat_settings.uv_transform =
            Affine2::from_scale_angle_translation(scale, modulo, translation);
    }
}
