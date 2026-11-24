// Portions of this shader are adapted from Potree (https://github.com/potree/potree)
// Copyright (c) 2011-2020, Markus Schütz
// Licensed under BSD 2-Clause (see THIRD_PARTY_LICENSES.md)

import bevy_pbr::render::mesh_view_bindings as view_bindings;
import bevy_pbr::render::mesh_functions::mesh_position_local_to_world;

import bevy_pbr::render::view_transformations::{
    position_view_to_clip,
    position_view_to_ndc,
    position_world_to_clip,
    position_world_to_view
};

import package::{
    materials::minimal::minimal_bindings as material_bindings,
    render::{
        functions,
        pointcloud_functions,
        forward_io::{InstanceInput, ShapeInput},
        pointcloud_bindings::pointcloud,
        pointcloud_types::{PointVertexPositions, PointVertexPositionsNormal}
    }
};

@if(IS_OCTREE)
import package::render::pointcloud_bindings::visible_nodes;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) view_position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) log_depth: f32,
    @location(4) radius: f32,
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

@if(IS_OCTREE)
fn is_bit_set(number: u32, index: u32) -> bool {
    return (number & (1u << index)) != 0u;
}

@if(IS_OCTREE)
fn count_one_bits_compat(x: u32) -> u32 {
    var v = x;
    v = v - ((v >> 1u) & 0x55555555u);
    v = (v & 0x33333333u) + ((v >> 2u) & 0x33333333u);
    return (((v + (v >> 4u)) & 0x0F0F0F0Fu) * 0x01010101u) >> 24u;
}

// Count number of bits before provided index
@if(IS_OCTREE)
fn count_bits_before(mask: u32, index: u32) -> u32 {
    // Create a mask for bits before index
    let before_mask = (1u << index) - 1u;

    // TODO add ifdef to use native version if available
    return count_one_bits_compat(mask & before_mask);
    // return countOneBits(mask & before_mask);
}

@if(IS_OCTREE)
fn get_max_relative_depth(position: vec3<f32>) -> f32 {
    // var current_index = visible_node.node_index;
    var current_index = 0u;
    var relative_depth: i32 = 0;

    var center = pointcloud.model_center.xyz;
    var half_extents = pointcloud.model_half_extents.xyz;

    for(var i = 0; i <= 30; i++) {
        let current_node = textureLoad(
                visible_nodes,
                vec2<u32>(current_index, pointcloud.octree_index),
                0,
            );

        // Extract data
        let children_mask = current_node.r; // u8 dans le canal R

        let first_child_index = current_node.b
            | (current_node.a << 8u); // u16 reconstruit à partir de B et A

        // Determiner in which octant is the position
        let relative_position = position - center.xyz;

        // index3d contains 0 or 1 for each axe
        let index3d = step(vec3(0.0), relative_position);

        // compute the child_index
        let child_index = u32(round(4.0 * index3d.x + 2.0 * index3d.y + index3d.z));

        // check if a children exists at this index
        if is_bit_set(children_mask, child_index) {
            // compute child offset
            var child_offset: u32 = 0u;
            if child_index > 0u {
                child_offset = count_bits_before(children_mask, child_index);
            }

            let actual_child_index = first_child_index + child_offset;

            relative_depth++;

            current_index = actual_child_index;
            half_extents = half_extents * 0.5;

            let offset = (index3d * 2.0 - 1.0) * half_extents;
            center = center + offset;
        } else {
            let offset = f32(current_node.g) / 10.0 - 10.0;
            return f32(relative_depth) + offset;
        }
    }

    return f32(relative_depth);
}

@vertex
fn vertex(shape: ShapeInput, vertex: InstanceInput) -> VertexOutput {
    let center = vertex.position.xyz;

    let viewport = view_bindings::view.viewport;

    let world_from_local = pointcloud_functions::get_world_from_local();

    let transform_scale = extract_max_scale(world_from_local);

    // Compute world & view position of the point instance (applying the world_from_local matrix)
    let world_position = mesh_position_local_to_world(
            world_from_local,
            vec4<f32>(vertex.position, 1.0),
        );
    var view_position = position_world_to_view(world_position.xyz);

    var radius: f32;
    @if(IS_OCTREE) {
        // Get the fov from projection matrix
        // let f = view_bindings::view.clip_from_view[1][1];
        // let fov = 2.0 * atan(1.0 / f);
        // let slope = tan(fov / 2.0);
        // var proj_factor = -0.5 * viewport[3] / (slope * view_position.z);

        let max_relative_depth = get_max_relative_depth(center);
        let attenuation = exp2(max_relative_depth);
        //let attenuation = pow(2.0, max_relative_depth);

        // Base screen-space radius driven by spacing and LOD attenuation
        // var radius_screen = pointcloud.spacing * 1.7 / attenuation;
        // radius_screen = radius_screen * proj_factor * transform_scale;

        // radius_screen = max(material.min_point_size, radius_screen);
        // radius_screen = min(material.max_point_size, radius_screen);
        // let radius = radius_screen / proj_factor;

        radius = pointcloud.spacing * transform_scale * 1.7 / attenuation;
    } @else {
        radius = pointcloud.point_size * transform_scale;
    }

    // Compute the offset to apply for creating a quad
    let offset = shape.position.xy * radius;

    // Apply the offset to the view position and compute clip position
    let clip_position = position_view_to_clip(view_position + vec3<f32>(offset, 0.0));

    var out: VertexOutput;

    out.clip_position = clip_position;
    out.view_position = view_position;

    @if(VERTEX_COLORS) {
        out.color = vertex.color * material_bindings::material.base_color;
    } @else {
        out.color = material_bindings::material.base_color;
    }

    out.log_depth = log2(-view_position.z);
    out.radius = radius;

    @if(SHAPE_UVS_A) {
        out.uv = shape.uv;
    } @else {
        out.uv = shape.position.xy + vec2(0.5);
    }

    @if(POINTCLOUD_PASS_DEPTH && WEIGHTED_SPLATS_ENABLED) {
        let original_depth = clip_position.w;
        let adjusted_depth = original_depth + 2.0 * radius;
        let adjust = adjusted_depth / original_depth;

        view_position *= adjust;
        view_position += vec3<f32>(offset, 0.0);

        out.clip_position = position_view_to_clip(view_position);
    }

    return out;
}

struct FragmentOutput {
    @if(POINTCLOUD_PASS_DEPTH && RENDER_MODE_EDL)
    @location(0) depth_texture: vec2<f32>,
    @if(POINTCLOUD_PASS_DEPTH && !RENDER_MODE_EDL)
    @location(0) depth_texture: f32,
    @if(!POINTCLOUD_PASS_DEPTH)
    @location(0) color: vec4<f32>,
    // @builtin(frag_depth) depth: f32,
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    // Perfect circle
    let diff = in.uv - vec2<f32>(0.5, 0.5);
    let dist_sq = dot(diff, diff);
    let radius_sq = pointcloud.radius * pointcloud.radius;

    if dist_sq > radius_sq {
        discard;
    }

    // convert the color to linear RGB
    let color = srgb_to_rgb_simple(in.color.xyz);

    var output: FragmentOutput;

    // output.depth = in.clip_position.z;

    @if(POINTCLOUD_PASS_DEPTH) {
        @if(RENDER_MODE_EDL) {
            output.depth_texture.r = in.clip_position.z;
            output.depth_texture.g = in.log_depth;
        } @else {
            output.depth_texture = in.clip_position.z;
        }
    } @else {
        @if(WEIGHTED_SPLATS_ENABLED) {
            let distance = sqrt(dist_sq);
            // var weight = max(0.0, 1.0 - distance * 2.0);
            var weight = max(0.0, 1.0 - distance * 2.0);
            weight = pow(weight, 1.5);

            output.color = vec4(color, weight);
        } @else {
            output.color = vec4(color, 1.0);
        }
    }

    return output;
}
