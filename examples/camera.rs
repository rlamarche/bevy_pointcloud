#![expect(missing_docs, reason = "Not all docs are written yet.")]

use std::f32::consts::TAU;

use bevy::{
    camera::ScalingMode,
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    color::palettes::css::RED,
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
        Transform::from_xyz(1.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        FreeCamera::default(),
    ));
}

fn load_point_cloud(
    mut commands: Commands,
    mut materials: ResMut<Assets<SimplePointCloudMaterial>>,
    point_cloud_server: Res<PointCloudServer>,
    asset_server: Res<AssetServer>,
) -> Result {
    let texture_handle = asset_server.load("branding/bevy_icon.png");
    commands.spawn((
        PointCloud3d(point_cloud_server.load::<PointCloudMeshLoader>(
            Sphere::new(0.5).mesh().ico(16)?,
        )),
        PointCloudMaterial3d(materials.add(SimplePointCloudMaterial {
            shape_radius: None,
            base_color: RED.into(),
            base_color_texture: Some(texture_handle),
            point_size: 0.01,
            ..default()
        })),
    ));

    Ok(())
}

fn update_scale(mut point_cloud: Query<&mut Transform, With<PointCloud3d>>, time: Res<Time<Real>>) {
    let modulo = (time.elapsed().as_millis() % 2000) as f32 * TAU / 2000.0;
    let modulo_2 = (time.elapsed().as_millis() % 4000) as f32 * TAU / 4000.0;

    let mut transform = point_cloud.single_mut().unwrap();

    *transform = Transform::from_scale(Vec3::new(
        1.0 + modulo.sin() / 2.0,
        1.0 + modulo_2.cos() / 2.0,
        1.0 + modulo.sin() / 2.0 + modulo.cos() / 4.0,
    ));
}

fn toggle_material(
    mut materials: ResMut<Assets<SimplePointCloudMaterial>>,
    material: Query<&PointCloudMaterial3d<SimplePointCloudMaterial>>,
    key_input: Res<ButtonInput<KeyCode>>,
) {
    if key_input.just_pressed(KeyCode::KeyP) {
        let material = material.single().unwrap();

        if let Some(mut material) = materials.get_mut(material) {
            if material.shape_radius.is_some() {
                material.shape_radius = None;
            } else {
                material.shape_radius = Some(0.50);
            }
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
                    viewport_height: 1.0,
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
