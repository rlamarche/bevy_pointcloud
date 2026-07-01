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

/// WIP function to handle adaptive point sizing in octrees
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

/// This function computes the world position of the point using the shape vertice
/// params:
/// - `position`: the position of the point (the instance)
/// - `world_from_local`: transform matrix from local (in the point cloud space) to the world
/// - `shape_position`: the position of a vertice of the shape
/// - `point_size`: the point size is juste a scale applied to the shape
/// Note that the scale from `world_from_local` is also applied (the max scale) to preserve coherent point sizing.
/// The shape is always face to the camera. Later we would be using the normal of the points to orient the shapes.
fn compute_world_position(
    position: vec3<f32>,
    world_from_local: mat4x4<f32>,
    shape_position: vec3<f32>,
    point_size: f32,
) -> vec3<f32> {
    var world_position = (world_from_local * vec4<f32>(position.xyz, 1.0)).xyz;
    let max_scale = extract_max_scale(world_from_local);

    let right  = vec3<f32>(view.world_from_view[0][0], view.world_from_view[0][1], view.world_from_view[0][2]);
    let up     = vec3<f32>(view.world_from_view[1][0], view.world_from_view[1][1], view.world_from_view[1][2]);
    let facing = vec3<f32>(view.world_from_view[2][0], view.world_from_view[2][1], view.world_from_view[2][2]);

    let billboard_shape = view.world_from_view * vec4<f32>(shape_position, 1.0);
    world_position = world_position + billboard_shape.xyz * max_scale * point_size;

    return world_position;
}
