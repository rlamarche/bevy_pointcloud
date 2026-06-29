#![expect(missing_docs, reason = "Not all docs are written yet.")]

use bevy::{
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    prelude::*,
    remote::{http::RemoteHttpPlugin, RemotePlugin},
    render::view::NoIndirectDrawing,
};
use bevy_pointcloud::{
    las::LasLoader, prelude::*, FileSource, PointCloud3d, PointCloudMaterial3d, PointCloudServer,
    StandardPointCloudMaterial,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins((RemotePlugin::default(), RemoteHttpPlugin::default()))
        .add_plugins(FreeCameraPlugin)
        .add_plugins(PointCloudPlugin::default())
        .add_systems(Startup, (setup, load_point_cloud))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 5.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
        NoIndirectDrawing,
        Msaa::Off,
        FreeCamera::default(),
    ));
}

fn load_point_cloud(
    // mut materials: ResMut<Assets<StandardMaterial>>,
    mut pc_materials: ResMut<Assets<StandardPointCloudMaterial>>,
    point_cloud_server: Res<PointCloudServer>,
    // mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
) -> Result {
    // let debug_material = materials.add(StandardMaterial {
    //     // base_color: Color::WHITE,
    //     base_color_texture: Some(images.add(uv_debug_texture())),
    //     ..default()
    // });

    let debug_pc_material = pc_materials.add(StandardPointCloudMaterial {
        base_color: Color::WHITE,
        // base_color_texture: Some(images.add(uv_debug_texture())),
        ..default()
    });

    let loader = LasLoader::from(FileSource::open(
        "assets/pointclouds/lion_takanawa.copc.laz",
    )?);

    let point_cloud_handle = point_cloud_server.load(loader);
    commands.spawn((
        PointCloud3d(point_cloud_handle),
        Transform::from_rotation(Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2)),
        // MeshMaterial3d(debug_material.clone()),
        PointCloudMaterial3d(debug_pc_material.clone()),
    ));

    Ok(())
}

// /// Creates a colorful test pattern
// fn uv_debug_texture() -> Image {
//     const TEXTURE_SIZE: usize = 8;

//     let mut palette: [u8; 32] = [
//         255, 102, 159, 255, 255, 159, 102, 255, 236, 255, 102, 255, 121, 255, 102, 255, 102, 255,
//         198, 255, 102, 198, 255, 255, 121, 102, 255, 255, 236, 102, 255, 255,
//     ];

//     let mut texture_data = [0; TEXTURE_SIZE * TEXTURE_SIZE * 4];
//     for y in 0..TEXTURE_SIZE {
//         let offset = TEXTURE_SIZE * y * 4;
//         texture_data[offset..(offset + TEXTURE_SIZE * 4)].copy_from_slice(&palette);
//         palette.rotate_right(4);
//     }

//     Image::new_fill(
//         Extent3d {
//             width: TEXTURE_SIZE as u32,
//             height: TEXTURE_SIZE as u32,
//             depth_or_array_layers: 1,
//         },
//         TextureDimension::D2,
//         &texture_data,
//         TextureFormat::Rgba8UnormSrgb,
//         RenderAssetUsages::RENDER_WORLD,
//     )
// }
