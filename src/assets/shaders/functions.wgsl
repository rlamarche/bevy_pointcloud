#define_import_path bevy_pointcloud::functions

#import bevy_pbr::mesh_view_bindings::view
#import bevy_pointcloud::{
    types,
    material_types::ColorStop,
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


/// This function computes the world position of the point using the 3D shape vertex oriented by the point's normal.
/// The missing tangent will be computed automatically.
/// params:
/// - `position`: the position of the point (the instance)
/// - `normal`: the local normal vector of the point (the instance)
/// - `world_from_local`: transform matrix from local (in the point cloud space) to the world
/// - `shape_position`: the 3D position of a vertex of the shape (e.g. vertices of a sphere)
/// - `point_size`: the point size is just a scale applied to the shape
fn compute_world_position_oriented(
    position: vec3<f32>,
    normal: vec3<f32>,
    world_from_local: mat4x4<f32>,
    shape_position: vec3<f32>,
    point_size: f32,
) -> vec3<f32> {
    // 1. Compute the world position of the point instance center
    var world_position = (world_from_local * vec4<f32>(position, 1.0)).xyz;
    let max_scale = extract_max_scale(world_from_local);

    // 2. Transform the local normal into world space and normalize it (Local Z-Axis)
    let world_normal = normalize((world_from_local * vec4<f32>(normal, 0.0)).xyz);

    // 3. Build the remaining orthonormal basis (Tangent & Bitangent)
    var up_ref = vec3<f32>(0.0, 1.0, 0.0);
    if (abs(world_normal.y) > 0.99) {
        up_ref = vec3<f32>(1.0, 0.0, 0.0);
    }

    // World tangent (Local X-Axis)
    let world_tangent = cross(up_ref, world_normal);
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
    world_position = world_position + offset * max_scale * point_size;

    return world_position;
}

/// This function computes the world position of the point using the 3D shape vertex oriented by the point's normal and tangent.
/// params:
/// - `position`: the position of the point (the instance)
/// - `normal`: the local normal vector of the point (the instance)
/// - `tangent`: the local tagent vector of the point (the instance)
/// - `world_from_local`: transform matrix from local (in the point cloud space) to the world
/// - `shape_position`: the 3D position of a vertex of the shape (e.g. vertices of a sphere)
/// - `point_size`: the point size is just a scale applied to the shape
fn compute_world_position_oriented_with_tangent(
    position: vec3<f32>,
    normal: vec3<f32>,
    tangent: vec3<f32>,
    world_from_local: mat4x4<f32>,
    shape_position: vec3<f32>,
    point_size: f32,
) -> vec3<f32> {
    // Compute the world position of the point instance center
    var world_position = (world_from_local * vec4<f32>(position, 1.0)).xyz;
    let max_scale = extract_max_scale(world_from_local);

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
    world_position = world_position + offset * max_scale * point_size;

    return world_position;
}

// Function to evaluate the multi-stop gradient based on shaderdefs
fn evaluate_gradient(base_color: vec4<f32>, end_color: vec4<f32>, color_stops: array<ColorStop, 8>, t: f32) -> vec4<f32> {
    let factor = clamp(t, 0.0, 1.0);

    #ifdef SIMPLE_MATERIAL_GRADIENT
        var current_start_color = base_color;
        var current_start_point = 0.0;

        // Cumulative chain for 8 color stops
        #ifdef SIMPLE_MATERIAL_COLOR_STOP_1
            if factor < color_stops[0].point {
                let local_t = (factor - current_start_point) / (color_stops[0].point - current_start_point);
                return mix(current_start_color, color_stops[0].color, local_t);
            }
            current_start_color = color_stops[0].color;
            current_start_point = color_stops[0].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_2
            if factor < color_stops[1].point {
                let local_t = (factor - current_start_point) / (color_stops[1].point - current_start_point);
                return mix(current_start_color, color_stops[1].color, local_t);
            }
            current_start_color = color_stops[1].color;
            current_start_point = color_stops[1].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_3
            if factor < color_stops[2].point {
                let local_t = (factor - current_start_point) / (color_stops[2].point - current_start_point);
                return mix(current_start_color, color_stops[2].color, local_t);
            }
            current_start_color = color_stops[2].color;
            current_start_point = color_stops[2].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_4
            if factor < color_stops[3].point {
                let local_t = (factor - current_start_point) / (color_stops[3].point - current_start_point);
                return mix(current_start_color, color_stops[3].color, local_t);
            }
            current_start_color = color_stops[3].color;
            current_start_point = color_stops[3].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_5
            if factor < color_stops[4].point {
                let local_t = (factor - current_start_point) / (color_stops[4].point - current_start_point);
                return mix(current_start_color, color_stops[4].color, local_t);
            }
            current_start_color = color_stops[4].color;
            current_start_point = color_stops[4].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_6
            if factor < color_stops[5].point {
                let local_t = (factor - current_start_point) / (color_stops[5].point - current_start_point);
                return mix(current_start_color, color_stops[5].color, local_t);
            }
            current_start_color = color_stops[5].color;
            current_start_point = color_stops[5].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_7
            if factor < color_stops[6].point {
                let local_t = (factor - current_start_point) / (color_stops[6].point - current_start_point);
                return mix(current_start_color, color_stops[6].color, local_t);
            }
            current_start_color = color_stops[6].color;
            current_start_point = color_stops[6].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_8
            if factor < color_stops[7].point {
                let local_t = (factor - current_start_point) / (color_stops[7].point - current_start_point);
                return mix(current_start_color, color_stops[7].color, local_t);
            }
            current_start_color = color_stops[7].color;
            current_start_point = color_stops[7].point;
        #endif

        // Final interpolation between the last active stop and end_color
        let final_t = (factor - current_start_point) / (1.0 - current_start_point);
        return mix(current_start_color, end_color, final_t);

    #else
        return base_color;
    #endif
}
