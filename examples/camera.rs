#![expect(missing_docs, reason = "Not all docs are written yet.")]

use std::f32::consts::TAU;

use bevy::{
    camera::ScalingMode,
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    color::palettes::css::{RED, YELLOW},
    prelude::*,
};
use bevy_pointcloud::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FreeCameraPlugin)
        .add_plugins(PointCloudPlugin::default())
        .add_systems(Startup, (setup, load_point_cloud))
        .add_systems(PreUpdate, update_scale)
        .add_systems(PostUpdate, (toggle_projection, toggle_material))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-4.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        FreeCamera::default(),
    ));
}

fn load_point_cloud(
    mut commands: Commands,
    mut materials: ResMut<Assets<SimplePointCloudMaterial>>,
    mut std_materials: ResMut<Assets<StandardPointCloudMaterial>>,
    point_cloud_server: Res<PointCloudServer>,
    asset_server: Res<AssetServer>,
) -> Result {
    let texture_handle = asset_server.load("branding/bevy_icon.png");
    let point_cloud =
        point_cloud_server.load::<PointCloudMeshLoader>(Sphere::new(0.5).mesh().ico(16)?);
    commands.spawn((
        PointCloud3d(point_cloud.clone()),
        SplatSettings {
            point_size_mode: PointSizeMode::WorldSpace,
            point_size: 0.02, // in meters
            ..default()
        },
        PointCloudMaterial3d(materials.add(SimplePointCloudMaterial {
            base_color: RED.into(),
            base_color_texture: Some(texture_handle.clone()),
            ..default()
        })),
        Transform::from_translation(Vec3::new(0.0, -1.0, -1.0)),
    ));

    let mut pbr_material: StandardPointCloudMaterial = Color::from(RED).into();
    pbr_material.base_color_texture = Some(texture_handle.clone());

    commands.spawn((
        PointCloud3d(point_cloud.clone()),
        SplatSettings {
            orientation: SplatOrientation::FaceNormal,
            point_size_mode: PointSizeMode::LocalSpace,
            point_size: 0.025,
            // min_point_size: Some(10.0),
            // max_point_size: Some(30.0),
            ..default()
        },
        // PointCloudMaterial3d(materials.add(SimplePointCloudMaterial {
        //     shape_radius: None,
        //     base_color: RED.into(),
        //     base_color_texture: Some(texture_handle.clone()),
        //     point_size_mode: PointSizeMode::LocalSpace,
        //     point_size: 0.02, // in meters
        //     ..default()
        // })),
        PointCloudMaterial3d(std_materials.add(pbr_material)),
        Transform::from_translation(Vec3::new(0.0, -1.0, 1.0)),
    ));

    commands.spawn((
        PointCloud3d(point_cloud.clone()),
        SplatSettings {
            point_size_mode: PointSizeMode::WorldSpace,
            point_size: 0.02, // in meters
            ..default()
        },
        PointCloudMaterial3d(materials.add(SimplePointCloudMaterial {
            base_color: YELLOW.into(),
            base_color_texture: Some(texture_handle.clone()),

            ..default()
        })),
        Transform::from_translation(Vec3::new(0.0, 1.0, -1.0)),
    ));

    commands.spawn((
        PointCloud3d(point_cloud.clone()),
        SplatSettings {
            point_size_mode: PointSizeMode::ScreenPixelsLocal,
            point_size: 20.0, /* in screen pixels (which fade with distance in perspective
                               * projection) */
            ..default()
        },
        PointCloudMaterial3d(materials.add(SimplePointCloudMaterial {
            base_color: YELLOW.into(),
            base_color_texture: Some(texture_handle.clone()),

            ..default()
        })),
        Transform::from_translation(Vec3::new(0.0, 1.0, 1.0)),
    ));

    Ok(())
}

fn update_scale(mut point_cloud: Query<&mut Transform, With<PointCloud3d>>, time: Res<Time<Real>>) {
    let modulo = (time.elapsed().as_millis() % 2000) as f32 * TAU / 2000.0;
    let modulo_2 = (time.elapsed().as_millis() % 4000) as f32 * TAU / 4000.0;

    for mut transform in point_cloud.iter_mut() {
        *transform = transform.with_scale(Vec3::new(
            1.0 + modulo.sin() / 2.0,
            1.0 + modulo_2.cos() / 2.0,
            1.0 + modulo.sin() / 2.0 + modulo.cos() / 4.0,
        ));
    }
}

fn toggle_material(
    mut splat_settings: Query<&mut SplatSettings>,
    key_input: Res<ButtonInput<KeyCode>>,
) {
    if key_input.just_pressed(KeyCode::KeyP) {
        let mut splat_settings = splat_settings.single_mut().unwrap();

        if splat_settings.radius.is_some() {
            splat_settings.radius = None;
        } else {
            splat_settings.radius = Some(0.50);
        }
    }
}

fn toggle_projection(
    mut camera: Query<&mut Projection, With<Camera>>,
    key_input: Res<ButtonInput<KeyCode>>,
) {
    if key_input.just_pressed(KeyCode::Space) {
        let mut projection = camera.single_mut().unwrap();

        let next_projection = match projection.as_ref() {
            Projection::Perspective(_) => Projection::Orthographic(OrthographicProjection {
                scale: 1.0,
                near: 0.0,
                far: 1000.0,
                viewport_origin: Vec2::new(0.5, 0.5),
                scaling_mode: ScalingMode::FixedVertical {
                    viewport_height: 5.0,
                },
                area: Rect::new(-1.0, -1.0, 1.0, 1.0),
            }),
            Projection::Orthographic(_) | Projection::Custom(_) => {
                Projection::Perspective(Default::default())
            }
        };

        *projection = next_projection;
    }
}
