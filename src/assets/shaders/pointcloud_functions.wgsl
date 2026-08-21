#define_import_path bevy_pointcloud::pointcloud_functions

#import bevy_render::view::position_view_to_world;

#import bevy_pbr::{
    mesh_view_bindings::{
        view,
        visibility_ranges,
        VISIBILITY_RANGE_UNIFORM_BUFFER_SIZE,
    },
    mesh_types::MESH_FLAGS_SIGN_DETERMINANT_MODEL_3X3_BIT,
    view_transformations::{
        position_world_to_view,
        position_world_to_clip,
        position_view_to_clip,
    }
}
#import bevy_render::maths::{affine3_to_square, mat2x4_f32_to_mat3x3_unpack}

#import bevy_pointcloud::{
    functions,
    pointcloud_bindings::pointcloud,
    pointcloud_types::{
        PointVertexPositions,
        PointVertexPositionsNormal,
    }
}

#ifdef IS_OCTREE
#import bevy_pointcloud::pointcloud_bindings::visible_nodes
#endif // IS_OCTREE

fn get_world_from_local() -> mat4x4<f32> {
    return affine3_to_square(pointcloud.world_from_local);
}

fn get_previous_world_from_local() -> mat4x4<f32> {
    return affine3_to_square(pointcloud.previous_world_from_local);
}

fn get_local_from_world() -> mat4x4<f32> {
    // the model matrix is translation * rotation * scale
    // the inverse is then scale^-1 * rotation ^-1 * translation^-1
    // the 3x3 matrix only contains the information for the rotation and scale
    let inverse_model_3x3 = transpose(mat2x4_f32_to_mat3x3_unpack(
        pointcloud.local_from_world_transpose_a,
        pointcloud.local_from_world_transpose_b,
    ));
    // construct scale^-1 * rotation^-1 from the 3x3
    let inverse_model_4x4_no_trans = mat4x4<f32>(
        vec4(inverse_model_3x3[0], 0.0),
        vec4(inverse_model_3x3[1], 0.0),
        vec4(inverse_model_3x3[2], 0.0),
        vec4(0.0,0.0,0.0,1.0)
    );
    // we can get translation^-1 by negating the translation of the model
    let model = get_world_from_local();
    let inverse_model_4x4_only_trans = mat4x4<f32>(
        vec4(1.0,0.0,0.0,0.0),
        vec4(0.0,1.0,0.0,0.0),
        vec4(0.0,0.0,1.0,0.0),
        vec4(-model[3].xyz, 1.0)
    );

    return inverse_model_4x4_no_trans * inverse_model_4x4_only_trans;
}


fn mesh_position_local_to_world(world_from_local: mat4x4<f32>, vertex_position: vec4<f32>) -> vec4<f32> {
    return world_from_local * vertex_position;
}

// NOTE: The intermediate world_position assignment is important
// for precision purposes when using the 'equals' depth comparison
// function.
fn mesh_position_local_to_clip(world_from_local: mat4x4<f32>, vertex_position: vec4<f32>) -> vec4<f32> {
    let world_position = mesh_position_local_to_world(world_from_local, vertex_position);
    return position_world_to_clip(world_position.xyz);
}


fn mesh_tangent_local_to_world(world_from_local: mat4x4<f32>, vertex_tangent: vec4<f32>) -> vec4<f32> {
    // NOTE: The mikktspace method of normal mapping requires that the world tangent is
    // re-normalized in the vertex shader to match the way mikktspace bakes vertex tangents
    // and normal maps so that the exact inverse process is applied when shading. Blender, Unity,
    // Unreal Engine, Godot, and more all use the mikktspace method.
    // We only skip normalization for invalid tangents so that they don't become NaN.
    // Do not change this code unless you really know what you are doing.
    // http://www.mikktspace.com/
    if any(vertex_tangent != vec4<f32>(0.0)) {
        return vec4<f32>(
            normalize(
                mat3x3<f32>(
                    world_from_local[0].xyz,
                    world_from_local[1].xyz,
                    world_from_local[2].xyz,
                ) * vertex_tangent.xyz
            ),
            // NOTE: Multiplying by the sign of the determinant of the 3x3 model matrix accounts for
            // situations such as negative scaling.
            vertex_tangent.w * sign_determinant_model_3x3m(pointcloud.mesh_flags)
        );
    } else {
        return vertex_tangent;
    }
}

// Calculates the sign of the determinant of the 3x3 model matrix based on a
// mesh flag
fn sign_determinant_model_3x3m(mesh_flags: u32) -> f32 {
    // bool(u32) is false if 0u else true
    // f32(bool) is 1.0 if true else 0.0
    // * 2.0 - 1.0 remaps 0.0 or 1.0 to -1.0 or 1.0 respectively
    return f32(bool(mesh_flags & MESH_FLAGS_SIGN_DETERMINANT_MODEL_3X3_BIT)) * 2.0 - 1.0;
}


// Returns an appropriate dither level for the current mesh instance.
//
// This looks up the LOD range in the `visibility_ranges` table and compares the
// camera distance to determine the dithering level.
#ifdef VISIBILITY_RANGE_DITHER
fn get_visibility_range_dither_level(world_position: vec4<f32>) -> i32 {
#if AVAILABLE_STORAGE_BUFFER_BINDINGS >= 6
    // If we're using a storage buffer, then the length is variable.
    let visibility_buffer_array_len = arrayLength(&visibility_ranges);
#else   // AVAILABLE_STORAGE_BUFFER_BINDINGS >= 6
    // If we're using a uniform buffer, then the length is constant
    let visibility_buffer_array_len = VISIBILITY_RANGE_UNIFORM_BUFFER_SIZE;
#endif  // AVAILABLE_STORAGE_BUFFER_BINDINGS >= 6

    let visibility_buffer_index = pointcloud.mesh_flags & 0xffffu;
    if (visibility_buffer_index > visibility_buffer_array_len) {
        return -16;
    }

    let lod_range = visibility_ranges[visibility_buffer_index];
    let camera_distance = length(view.lod_view_world_position.xyz - world_position.xyz);

    // This encodes the following mapping:
    //
    //     `lod_range.`          x        y        z        w           camera distance
    //                   ←───────┼────────┼────────┼────────┼────────→
    //     Dither Level  -16    -16       0        0        16      16  Dither Level
    let offset = select(-16, 0, camera_distance >= lod_range.z);
    let bounds = select(lod_range.xy, lod_range.zw, camera_distance >= lod_range.z);
    let level = i32(round((camera_distance - bounds.x) / (bounds.y - bounds.x) * 16.0));
    return offset + clamp(level, 0, 16);
}
#endif


/// Computes the final splat/point radius in World Space or Screen Space based on
/// the configured size mode (`PointSizeMode`) and optional adaptive size bounds.
///
/// # Arguments
/// * `position` - The raw position of the point in the point cloud (not transformed). Used only for adaptive point sizing.
/// * `world_from_local` - The 4x3 world-from-local transformation matrix of the point cloud.
/// * `view_position` - The point/vertex position in view or world space.
fn compute_point_radius(
    position: vec3<f32>,
    world_from_local: mat4x4<f32>,
    view_position: vec3<f32>,
) -> f32 {
    var radius: f32 = 0.0;
    let radius_scale = functions::extract_max_scale(world_from_local);

    let min_size = pointcloud.min_point_size;
    let max_size = pointcloud.max_point_size;

    #ifdef POINT_SIZE_MODE_SCREEN_PIXELS
        radius = functions::compute_screen_space_point_size(
            view_position,
            pointcloud.point_size,
            min_size,
            max_size
        );
    #endif

    #ifdef POINT_SIZE_MODE_SCREEN_LOCAL
        radius = functions::compute_screen_space_point_size(
            view_position,
            pointcloud.point_size * radius_scale,
            min_size,
            max_size
        );
    #endif

    #ifdef POINT_SIZE_MODE_WORLD
        radius = functions::compute_world_space_point_size(
            view_position,
            pointcloud.point_size,
            min_size,
            max_size
        );
    #endif

    #ifdef POINT_SIZE_MODE_LOCAL
        radius = functions::compute_world_space_point_size(
            view_position,
            pointcloud.point_size * radius_scale,
            min_size,
            max_size
        );
    #endif

    #ifdef ADAPTIVE_POINT_SIZE
        #ifdef IS_OCTREE
            let max_relative_depth = get_max_relative_depth(position);
            let attenuation = exp2(max_relative_depth);
            let spacing = select(1.0, pointcloud.spacing, pointcloud.spacing > 0.0);
            radius = radius * spacing * 1.7 / attenuation;
        #endif // IS_OCTREE
    #endif // ADAPTIVE_POINT_SIZE

    return radius;
}


/// Computes all vertex positions (world, view, clip) from the point center world position.
fn compute_point_vertex_positions(
    point_world_position: vec3<f32>,
    world_from_local: mat4x4<f32>,
    shape_position: vec3<f32>,
    radius: f32,
    normal: vec3<f32>,
    tangent: vec3<f32>,
) -> PointVertexPositions {
    var res: PointVertexPositions;

    let point_view_position = position_world_to_view(point_world_position);

    #ifdef SPLAT_ORIENTATION_FACE_NORMAL
        #ifdef VERTEX_TANGENTS
            res.world_position =
                functions::compute_world_vertex_position_oriented_with_tangent(
                    point_world_position,
                    normal,
                    tangent,
                    world_from_local,
                    shape_position,
                    radius
                );
        #else
            res.world_position =
                functions::compute_world_vertex_position_oriented(
                    point_world_position,
                    normal,
                    world_from_local,
                    shape_position,
                    radius
                );
        #endif

        res.view_position = position_world_to_view(res.world_position);
    #else // Billboard mode
        res.view_position = functions::compute_view_billboard_vertex_position(
            point_view_position,
            shape_position,
            radius
        );

        res.world_position = position_view_to_world(res.view_position, view.world_from_view);
    #endif

    res.clip_position = position_view_to_clip(res.view_position);

    return res;
}




/// Computes vertex positions (world, view, clip) and world normal from the point center.
///
/// Conventions:
/// - `point_normal`: Surface normal of the point cloud sample in local/world space.
/// - `shape_normal`: Local normal of the current vertex from the splat geometry (mesh/cube/quad).
fn compute_point_vertex_positions_normal(
    point_world_position: vec3<f32>,
    world_from_local: mat4x4<f32>,
    shape_position: vec3<f32>,
    shape_normal: vec3<f32>,
    radius: f32,
    normal: vec3<f32>,
    tangent: vec3<f32>,
) -> PointVertexPositionsNormal {
    var res: PointVertexPositionsNormal;

    let point_view_position = position_world_to_view(point_world_position);

    #ifdef SPLAT_ORIENTATION_FACE_NORMAL
        #ifdef VERTEX_TANGENTS
            res.world_position =
                functions::compute_world_vertex_position_oriented_with_tangent(
                    point_world_position,
                    normal,
                    tangent,
                    world_from_local,
                    shape_position,
                    radius
                );

            // Orient shape local normal using the point TBN frame (Tangent, Bitangent, Normal)
            // and transform to world space.
            res.world_normal = functions::compute_world_vertex_normal_oriented_with_tangent(
                normal,
                tangent,
                world_from_local,
                shape_normal
            );
        #else
            res.world_position =
                functions::compute_world_vertex_position_oriented(
                    point_world_position,
                    normal,
                    world_from_local,
                    shape_position,
                    radius
                );

            // Orient shape local normal using constructed orthonormal basis from point normal.
            res.world_normal = functions::compute_world_vertex_normal_oriented(
                normal,
                world_from_local,
                shape_normal
            );
        #endif

        res.view_position = position_world_to_view(res.world_position);
    #else // Billboard mode
        res.view_position = functions::compute_view_billboard_vertex_position(
            point_view_position,
            shape_position,
            radius
        );

        res.world_position = position_view_to_world(res.view_position, view.world_from_view);

        // In billboard mode, rotate the shape local normal by the camera's orientation
        // (view_from_world transpose/inverse) so it stays aligned with the billboard frame.
        let world_from_view_rotation = mat3x3<f32>(
            view.world_from_view[0].xyz,
            view.world_from_view[1].xyz,
            view.world_from_view[2].xyz
        );
        res.world_normal = normalize(world_from_view_rotation * shape_normal);
    #endif

    res.clip_position = position_view_to_clip(res.view_position);

    return res;
}






/// Computes the final texture coordinates (UVs) for a point cloud vertex
/// according to the selected UV mapping mode and transformations.
///
/// # Parameters
/// * `vertex_uv`: Base UV coordinates from the point instance input.
/// * `shape_uv`: Geometry quad/shape UV coordinates.
/// * `vertex_normal`: Normal vector in local space.
/// * `vertex_tangent`: Tangent vector in local space.
/// * `world_vertex_position`: Computed position of the current quad vertex in World Space.
/// * `radius`: Final computed point radius in World Space.
///
/// # Returns
/// The computed 2D UV coordinates ready to be assigned to the vertex output.
fn compute_point_uv(
    vertex_uv: vec2<f32>,
    shape_uv: vec2<f32>,
    vertex_normal: vec3<f32>,
    vertex_tangent: vec3<f32>,
    world_vertex_position: vec3<f32>,
    radius: f32,
) -> vec2<f32> {
    var base_uv = vec2<f32>(0.5, 0.5); // Default fallback center

    #ifdef VERTEX_UVS
        base_uv = vertex_uv;
    #endif

    #ifdef UV_MAPPING_COMBINED
        let local_from_world = get_local_from_world();

        // Calculate local space radius
        let radius_vector_world = vec3<f32>(radius, 0.0, 0.0);
        let radius_vector_local = local_from_world * vec4<f32>(radius_vector_world, 0.0);
        let radius_local = length(radius_vector_local.xyz) * 0.5;

        var current_shape_uv = shape_uv;

        // Correct billboard flipping when moving camera to opposite side or upside down
        #ifndef SPLAT_ORIENTATION_FACE_NORMAL
            let view_right = vec3<f32>(1.0, 0.0, 0.0);
            let view_up    = vec3<f32>(0.0, 1.0, 0.0);

            let local_right = (local_from_world * (view.world_from_view * vec4<f32>(view_right, 0.0))).xyz;
            let local_up    = (local_from_world * (view.world_from_view * vec4<f32>(view_up, 0.0))).xyz;

            var billboard_tangent: vec3<f32>;
            var billboard_bitangent: vec3<f32>;

            #ifdef VERTEX_NORMALS
                #ifdef VERTEX_TANGENTS
                    billboard_tangent = vertex_tangent;
                #else
                    billboard_tangent = pointcloud.uv_u.xyz;
                #endif
                billboard_bitangent = cross(vertex_normal, billboard_tangent);
            #else
                billboard_tangent = pointcloud.uv_u.xyz;
                billboard_bitangent = pointcloud.uv_v.xyz;
            #endif

            // Check if axis flipping is required based on view vector
            let flip_u = dot(billboard_tangent, local_right) < 0.0;
            let flip_v = dot(billboard_bitangent, local_up) < 0.0;

            // Apply texture coordinate flip
            current_shape_uv.x = select(current_shape_uv.x, 1.0 - current_shape_uv.x, flip_u);
            current_shape_uv.y = select(current_shape_uv.y, 1.0 - current_shape_uv.y, flip_v);
        #endif // SPLAT_ORIENTATION_FACE_NORMAL

        // Compute combined base + shape footprint UVs
        base_uv = base_uv + (current_shape_uv - vec2<f32>(0.5)) * radius_local;
    #endif // UV_MAPPING_COMBINED

    #ifdef UV_MAPPING_PLANAR
        let local_from_world = get_local_from_world();
        let local_vertex_position = local_from_world * vec4<f32>(world_vertex_position, 1.0);

        let aabb_size = pointcloud.aabb_max.xyz - pointcloud.aabb_min.xyz;

        // Normalize vertex position within the bounding box
        var vertex_position_normalized = (local_vertex_position.xyz - pointcloud.aabb_min.xyz) / aabb_size;

        // Prevent zero divisions or NaN values by fallback centering
        vertex_position_normalized.x = select(vertex_position_normalized.x, 0.5, aabb_size.x == 0.0);
        vertex_position_normalized.y = select(vertex_position_normalized.y, 0.5, aabb_size.y == 0.0);
        vertex_position_normalized.z = select(vertex_position_normalized.z, 0.5, aabb_size.z == 0.0);

        let axis_u = pointcloud.uv_u.xyz;
        let axis_v = pointcloud.uv_v.xyz;

        base_uv = vec2<f32>(
            dot(axis_u, vertex_position_normalized),
            1.0 - dot(axis_v, vertex_position_normalized),
        );
    #endif // UV_MAPPING_PLANAR

    #ifdef UV_MAPPING_POINT_SHAPE
        base_uv = shape_uv;
    #endif // UV_MAPPING_POINT_SHAPE

    // Apply 2D affine transformation on global/planar UV coordinates
    #ifdef SPLAT_UV_TRANSFORM
        let uv_rot_scale = mat2x2<f32>(
            pointcloud.uv_transform_a.xy,
            pointcloud.uv_transform_a.zw
        );
        let uv_translation = pointcloud.uv_transform_b.xy;

        return (uv_rot_scale * base_uv) + uv_translation;
    #else
        return base_uv;
    #endif
}


#ifdef IS_OCTREE
fn get_max_relative_depth(
    position: vec3<f32>
) -> f32 {
    var current_index = 0u;
    var relative_depth: i32 = 0;

    var center = pointcloud.model_center.xyz;
    var half_extents = pointcloud.model_half_extents.xyz;

    for (var i = 0; i <= 30; i ++) {
        let current_node = textureLoad(visible_nodes, vec2<u32>(current_index, pointcloud.octree_index), 0);

        // Extract data
        let children_mask = current_node.r;  // u8 dans le canal R

        let first_child_index = current_node.b | (current_node.a << 8u);  // u16 reconstruit à partir de B et A

        // Determiner in which octant is the position
        let relative_position = position - center;

        // index3d contains 0 or 1 for each axe
        let index3d = step(vec3(0.0), relative_position);

        // compute the child_index
        let child_index = u32(round(4.0 * index3d.x + 2.0 * index3d.y + index3d.z));

        // check if a children exists at this index
        if functions::is_bit_set(children_mask, child_index) {
            // compute child offset
            var child_offset: u32 = 0u;
            if child_index > 0 {
                child_offset = functions::count_bits_before(children_mask, child_index);
            }

            let actual_child_index = first_child_index + child_offset;

            relative_depth ++;

            current_index = actual_child_index;
            half_extents = half_extents  * 0.5;

            let offset = (index3d * 2.0 - 1.0) * half_extents;
            center = center + offset;
        } else {
            let offset = f32(current_node.g) / 10.0 - 10.0;
            return f32(relative_depth) + offset;
        }

    }

    return f32(relative_depth);
}
#endif // IS_OCTREE
