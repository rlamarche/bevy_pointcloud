#define_import_path bevy_pointcloud::functions

#import bevy_pbr::mesh_view_bindings as view_bindings
#import bevy_pointcloud::{
    types,
    forward_io::InstanceInput,
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


fn compute_point_size(
    // will be used for adaptive point size
    vertex: InstanceInput,
    view_position: vec3<f32>,
    world_from_local: mat4x4<f32>,
    point_size: f32,
    // min_point_size: f32,
    // max_point_size: f32
) -> f32 {
    // let checked_max_point_size = select(max_point_size, F32_MAX, max_point_size <= 0.0);
    let transform_scale = extract_max_scale(world_from_local);
    let viewport = view_bindings::view.viewport;

    var radius_screen = point_size * transform_scale;
    // radius_screen = clamp(radius_screen, min_point_size, checked_max_point_size);
    //
    // Compute radius to size the point correctly with viewport size
    let radius = point_size / min(viewport[2], viewport[3]);

    return radius;
}
