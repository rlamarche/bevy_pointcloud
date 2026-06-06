#define_import_path bevy_pointcloud::functions

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


fn get_max_relative_depth(visible_nodes: texture_2d<u32>, position: vec3<f32>) -> f32 {
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
