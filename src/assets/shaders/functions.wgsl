#define_import_path bevy_pointcloud::functions

#import bevy_pbr::mesh_view_bindings::view
#import bevy_pointcloud::{
    types,
    forward_io::{ShapeInput, InstanceInput},
}

fn srgb_to_rgb_simple(color: vec3<f32>) -> vec3<f32> {
    return pow(color, vec3<f32>(2.2));
}

// Extract an approximate uniform scale factor from a transform matrix.
// We take the largest axis scale to keep point sizing stable under non-uniform scaling.
fn extract_max_scale(matrix: mat4x4<f32>) -> f32 {
    let scale_x = length(matrix[0].xyz);
    let scale_y = length(matrix[1].xyz);
    let scale_z = length(matrix[2].xyz);

    return max(scale_x, max(scale_y, scale_z));
}

// Extract the scale performed by this matrix transform
fn extract_scale(matrix: mat4x4<f32>) -> vec3<f32> {
    let scale_x = length(matrix[0].xyz);
    let scale_y = length(matrix[1].xyz);
    let scale_z = length(matrix[2].xyz);

    return vec3<f32>(scale_x, scale_y, scale_z);
}

fn compute_radius(
    // will be used for adaptive point size
    vertex: InstanceInput,
    // view_position: vec3<f32>,
    world_from_local: mat4x4<f32>,
    point_size: f32,
) -> f32 {
    let transform_scale = extract_max_scale(world_from_local);
    // let fov = view.clip_from_view[1][1];
    // let proj_factor = -0.5 * fov / view_position.z;

    // this cancel the projection size
    return point_size * transform_scale;
}

fn compute_world_position(
    vertex: InstanceInput,
    world_from_local: mat4x4<f32>,
    shape: ShapeInput,
    point_size: f32,
) -> vec3<f32> {
    // compute the point position in world
    var world_position = (world_from_local * vec4<f32>(vertex.position.xyz, 1.0)).xyz;
    let max_scale = extract_max_scale(world_from_local);

    let right  = vec3<f32>(view.world_from_view[0][0], view.world_from_view[0][1], view.world_from_view[0][2]);
    let up     = vec3<f32>(view.world_from_view[1][0], view.world_from_view[1][1], view.world_from_view[1][2]);
    let facing = vec3<f32>(view.world_from_view[2][0], view.world_from_view[2][1], view.world_from_view[2][2]);

    let billboard_shape = view.world_from_view * vec4<f32>(shape.position, 1.0);
    world_position = world_position + billboard_shape.xyz * max_scale * point_size;

    return world_position;

    // let billborad_shape = right * shape.position.x + up * shape.position.y + facing * shape.position.z;

    // TODO if normal available, compute 3d scale and shape projection using this normal

    // let scaled_shape = shape.position.xyz * max_scale * material.point_size;

    // world_position = world_position.xyz
    //     + right * scaled_shape.x
    //     + up * scaled_shape.y
    //     + facing * scaled_shape.z;
}
