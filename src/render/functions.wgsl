#define_import_path bevy_pointcloud::functions

#import bevy_pbr::mesh_view_bindings as view_bindings

#import bevy_pointcloud::types
#import bevy_pointcloud::bindings

const F32_MAX: f32 = 3.4028234663852886e+38;

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

#ifdef IS_OCTREE


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


fn get_max_relative_depth(octree_entity: types::OctreeEntity, octree_node: types::OctreeNode, visible_nodes: texture_2d<u32>, position: vec3<f32>) -> f32 {
    // var current_index = visible_node.node_index;
    var current_index = 0u;
    var relative_depth: i32 = 0;

    var center = octree_node.center;
    var half_extents = octree_node.half_extents;

    for (var i = 0; i <= 30; i ++) {
        let current_node = textureLoad(visible_nodes, vec2<u32>(current_index, octree_entity.octree_index), 0);

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
        if is_bit_set(children_mask, child_index) {
            // compute child offset
            var child_offset: u32 = 0u;
            if child_index > 0 {
                child_offset = count_bits_before(children_mask, child_index);
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

#endif


fn compute_point_size(
    vertex: types::Vertex,
    view_position: vec3<f32>,
    point_size: f32,
    min_point_size: f32,
    max_point_size: f32
) -> f32 {
    let checked_max_point_size = select(max_point_size, F32_MAX, max_point_size <= 0.0);
    let transform_scale = extract_max_scale(bindings::world_from_local);
    let viewport = view_bindings::view.viewport;

    #ifdef IS_OCTREE
        // Get the fov from projection matrix
        let f = view_bindings::view.clip_from_view[1][1];
        let fov = 2.0 * atan(1.0 / f);
        let slope = tan(fov / 2.0);
        var proj_factor = -0.5 * viewport[3] / (slope * view_position.z);

        let max_relative_depth = get_max_relative_depth(bindings::octree_entity, bindings::octree_node, bindings::visible_nodes, vertex.i_pos_size.xyz);
        let attenuation = exp2(max_relative_depth);

        // Base screen-space radius driven by spacing and LOD attenuation
        var radius_screen = bindings::octree_node.spacing * 1.7 / attenuation;
        radius_screen = radius_screen * proj_factor * transform_scale;
        radius_screen = clamp(radius_screen, min_point_size, checked_max_point_size);

        let radius = radius_screen / proj_factor;
    #else
        var radius_screen = select(point_size, vertex.i_pos_size.w, point_size <= 0.0) * transform_scale;
        radius_screen = clamp(radius_screen, min_point_size, checked_max_point_size);
        // Compute radius to size the point correctly with viewport size
        let radius = radius_screen / min(viewport[2], viewport[3]);
    #endif

    return radius;
}
