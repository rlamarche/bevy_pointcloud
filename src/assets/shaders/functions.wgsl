#define_import_path bevy_pointcloud::functions

#import bevy_pbr::{
    mesh_view_bindings::view,
    view_transformations::{
        position_world_to_clip,
        position_world_to_view,
        position_view_to_clip,
    },
}
#import bevy_pointcloud::{
    types,
    forward_io::{ShapeInput, InstanceInput},
}

const F32_MAX: f32 = 3.4028234663852886e+38;

fn clamp_point_size(point_size: f32, min: f32, max: f32) -> f32 {
    let checked_max = select(max, F32_MAX, max <= 0.0);

    return clamp(point_size, min, checked_max);
}


/// Computes the final World-Space quad size for a point configured in Screen-Space (pixel) units.
///
/// Projects the base pixel size to world space at the current view depth,
/// clamps the pixel footprint between min and max pixel bounds to prevent sub-pixel
/// disappearance or screen-filling overdraw, and returns the adjusted world-space size.
///
/// # Parameters
/// * `view_position`: Vertex position in view/camera space (uses `z` for depth).
/// * `point_size_px`: Base screen-space size of the point in pixels.
/// * `min_point_size_px`: Minimum allowed screen size in pixels.
/// * `max_point_size_px`: Maximum allowed screen size in pixels.
///
/// # Returns
/// The adjusted World-Space size for the quad geometry.
fn compute_screen_space_point_size(
    view_position: vec3<f32>,
    point_size_px: f32,
    min_point_size_px: f32,
    max_point_size_px: f32,
) -> f32 {
    let f = view.clip_from_view[1][1];
    let slope = 1.0 / f;
    let proj_factor = -0.5 * view.viewport[3] / (slope * view_position.z);
    var radius_screen = (point_size_px / min(view.viewport[2], view.viewport[3])) * proj_factor;

    radius_screen = clamp(radius_screen, min_point_size_px, max_point_size_px);

    return radius_screen / proj_factor;
}

/// Computes the final World-Space quad size for a point configured in World-Space units.
///
/// Projects the base world-space size to screen pixels at depth Z,
/// clamps the pixel footprint between min and max pixel bounds to prevent sub-pixel
/// disappearance or screen-filling overdraw, and returns the adjusted world-space size.
///
/// # Parameters
/// * `view_position`: Vertex position in view/camera space (uses `z` for depth).
/// * `point_size_world`: Base world-space size of the point.
/// * `min_point_size_px`: Minimum allowed screen size in pixels.
/// * `max_point_size_px`: Maximum allowed screen size in pixels.
///
/// # Returns
/// The adjusted World-Space size for the quad geometry.
fn compute_world_space_point_size(
    view_position: vec3<f32>,
    point_size_world: f32,
    min_point_size_px: f32,
    max_point_size_px: f32,
) -> f32 {
    let depth = max(abs(view_position.z), 0.0001);
    let proj_factor = (view.viewport.w * view.clip_from_view[1][1]) / (2.0 * depth);

    if (proj_factor < 0.0001) {
        return point_size_world;
    }

    // 1. Project world size to screen pixels at current depth
    let size_px = point_size_world * proj_factor;

    // 2. Always clamp resulting pixel size
    let clamped_px = clamp(size_px, min_point_size_px, max_point_size_px);

    // 3. Convert back to world space size for quad geometry
    return clamped_px / proj_factor;
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


/// This function computes the view position of the point using the shape vertice, in billboard mode.
/// params:
/// - `position`: the position of the point (the instance)
/// - `world_from_local`: transform matrix from local (in the point cloud space) to the world
/// - `shape_position`: the position of a vertice of the shape
/// - `point_size`: the point size is juste a scale applied to the shape
/// Note that the scale from `world_from_local` is also applied (the max scale) to preserve coherent point sizing.
/// The shape is always face to the camera. Later we would be using the normal of the points to orient the shapes.
fn compute_view_billboard_vertex_position(
    view_position: vec3<f32>,
    shape_position: vec3<f32>,
    radius: f32,
) -> vec3<f32> {
    let offset = shape_position * radius;

    return view_position + offset;
}

/// This function computes the world position of the point using the 3D shape vertex oriented by the point's normal.
/// The missing tangent will be computed automatically.
/// params:
/// - `world_position`: the position of the point in the world
/// - `normal`: the local normal vector of the point (the instance)
/// - `world_from_local`: transform matrix from local (in the point cloud space) to the world
/// - `shape_position`: the 3D position of a vertex of the shape (e.g. vertices of a sphere)
/// - `point_size`: the point size is just a scale applied to the shape
fn compute_world_vertex_position_oriented(
    world_position: vec3<f32>,
    normal: vec3<f32>,
    world_from_local: mat4x4<f32>,
    shape_position: vec3<f32>,
    radius: f32,
) -> vec3<f32> {
    // Transform the local normal into world space and normalize it (Local Z-Axis)
    let world_normal = normalize((world_from_local * vec4<f32>(normal, 0.0)).xyz);

    // Build the remaining orthonormal basis (Tangent & Bitangent)
    var up_ref = vec3<f32>(0.0, 1.0, 0.0);
    if (abs(world_normal.y) > 0.99) {
        up_ref = vec3<f32>(1.0, 0.0, 0.0);
    }

    // World tangent (Local X-Axis)
    // up_ref and world_normal may not be orthogonal, that's why normalize is needed.
    let world_tangent = normalize(cross(up_ref, world_normal));
    // World bitangent (Local Y-Axis)
    let world_bitangent = cross(world_normal, world_tangent);

    // Offset the instance center using all 3 axes of the shape
    // - shape_position.x moves along the Tangent (X)
    // - shape_position.y moves along the Bitangent (Y)
    // - shape_position.z moves along the Normal (Z)
    let offset = (world_tangent * shape_position.x)
               + (world_bitangent * shape_position.y)
               + (world_normal * shape_position.z);

    // Apply the scale and point size to the 3D offset, then add to the center
    let world_vertex_position = world_position + offset * radius;

    return world_vertex_position;
}




/// This function computes the world position of the point using the 3D shape vertex oriented by the point's normal and tangent.
/// params:
/// - `world_position`: the position of the point in the world
/// - `normal`: the local normal vector of the point (the instance)
/// - `tangent`: the local tagent vector of the point (the instance)
/// - `world_from_local`: transform matrix from local (in the point cloud space) to the world
/// - `shape_position`: the 3D position of a vertex of the shape (e.g. vertices of a sphere)
/// - `point_size`: the point size is just a scale applied to the shape
fn compute_world_vertex_position_oriented_with_tangent(
    world_position: vec3<f32>,
    normal: vec3<f32>,
    tangent: vec3<f32>,
    world_from_local: mat4x4<f32>,
    shape_position: vec3<f32>,
    radius: f32,
) -> vec3<f32> {
    // Transform the local normal into world space and normalize it (Local Z-Axis)
    let world_normal = normalize((world_from_local * vec4<f32>(normal, 0.0)).xyz);

    // World tangent (Local X-Axis)
    let world_tangent = normalize((world_from_local * vec4<f32>(tangent, 0.0)).xyz);

    // World bitangent (Local Y-Axis)
    let world_bitangent = cross(world_normal, world_tangent);

    // Offset the instance center using all 3 axes of the shape
    // - shape_position.x moves along the Tangent (X)
    // - shape_position.y moves along the Bitangent (Y)
    // - shape_position.z moves along the Normal (Z) -> Crutial for 3D shapes like spheres!
    let offset = (world_tangent * shape_position.x)
               + (world_bitangent * shape_position.y)
               + (world_normal * shape_position.z);

    // Apply the scale and point size to the 3D offset, then add to the center
    let world_vertex_position = world_position + offset * radius;

    return world_vertex_position;
}


fn compute_tangent_and_bitangent(
    normal: vec3<f32>,
) -> mat2x3<f32> {
    // Build the remaining orthonormal basis (Tangent & Bitangent)
    var up_ref = vec3<f32>(0.0, 1.0, 0.0);
    // TODO replace with select ?
    if (abs(normal.y) > 0.99) {
        up_ref = vec3<f32>(1.0, 0.0, 0.0);
    }

    let tangent = normalize(cross(up_ref, normal));
    let bitangent = cross(normal, tangent);

    return mat2x3<f32>(tangent, bitangent);
}



fn compute_world_vertex_normal_oriented_with_tangent(
    normal: vec3<f32>,
    tangent: vec3<f32>,
    world_from_local: mat4x4<f32>,
    shape_normal: vec3<f32>,
) -> vec3<f32> {
    let N = normalize(normal);
    let T = normalize(tangent);
    let B = normalize(cross(N, T));

    let tbn = mat3x3<f32>(T, B, N);
    let local_normal = tbn * shape_normal;

    let world_rot = mat3x3<f32>(
        world_from_local[0].xyz,
        world_from_local[1].xyz,
        world_from_local[2].xyz
    );

    return normalize(world_rot * local_normal);
}

fn compute_world_vertex_normal_oriented(
    normal: vec3<f32>,
    world_from_local: mat4x4<f32>,
    shape_normal: vec3<f32>,
) -> vec3<f32> {
    let world_normal = normalize((world_from_local * vec4<f32>(normal, 0.0)).xyz);
    var up_ref = vec3<f32>(0.0, 1.0, 0.0);
    if (abs(world_normal.y) > 0.99) {
        up_ref = vec3<f32>(1.0, 0.0, 0.0);
    }

    let world_tangent = normalize(cross(up_ref, world_normal));
    let world_bitangent = cross(world_normal, world_tangent);

    let final_world_normal = (world_tangent * shape_normal.x)
                               + (world_bitangent * shape_normal.y)
                               + (world_normal * shape_normal.z);

    return normalize(final_world_normal);

    // let N = normalize(normal);

    // // Choose an axis that is not parallel to N to construct a stable tangent basis
    // let up = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), abs(N.z) < 0.999);
    // let T = normalize(cross(up, N));
    // let B = cross(N, T);

    // let tbn = mat3x3<f32>(T, B, N);
    // let local_normal = tbn * shape_normal;

    // let world_rot = mat3x3<f32>(
    //     world_from_local[0].xyz,
    //     world_from_local[1].xyz,
    //     world_from_local[2].xyz
    // );

    // return normalize(world_rot * local_normal);
}



fn is_bit_set(number: u32, index: u32) -> bool {
    return (number & (1u << index)) != 0u;
}

fn count_one_bits_compat(x: u32) -> u32 {
    var v = x;
    v = v - ((v >> 1u) & 0x55555555u);
    v = (v & 0x33333333u) + ((v >> 2u) & 0x33333333u);
    return (((v + (v >> 4u)) & 0x0F0F0F0Fu) * 0x01010101u) >> 24u;
}

// Count number of bits before provided index
fn count_bits_before(mask: u32, index: u32) -> u32 {
    // Create a mask for bits before index
    let before_mask = (1u << index) - 1u;

    // TODO add ifdef to use native version if available
    return count_one_bits_compat(mask & before_mask);
//    return countOneBits(mask & before_mask);
}
