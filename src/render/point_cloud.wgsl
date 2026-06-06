// Portions of this shader are adapted from Potree (https://github.com/potree/potree)
// Copyright (c) 2011-2020, Markus Schütz
// Licensed under BSD 2-Clause (see THIRD_PARTY_LICENSES.md)

#import bevy_pbr::mesh_view_bindings as view_bindings
#import bevy_pbr::mesh_functions::mesh_position_local_to_world

#import bevy_pbr::view_transformations::position_world_to_clip
#import bevy_pbr::view_transformations::position_world_to_view
#import bevy_pbr::view_transformations::position_view_to_clip
#import bevy_pbr::view_transformations::position_view_to_ndc

#import bevy_pointcloud::bindings
#import bevy_pointcloud::functions
#import bevy_pointcloud::types


struct PointCloudMaterial {
    point_size: f32,
    min_point_size: f32,
    max_point_size: f32,
#ifdef SIXTEEN_BYTE_ALIGNMENT
    // WebGL2 structs must be 16 byte aligned.
    _webgl2_padding: f32,
#endif
};

@group(2) @binding(0)
var<uniform> material: PointCloudMaterial;


@vertex
fn vertex(vertex: types::Vertex) -> types::VertexOutput {
    // Compute world & view position of the point instance (applying the bindings::world_from_local matrix)
    let world_position = mesh_position_local_to_world(bindings::world_from_local, vec4<f32>(vertex.i_pos_size.xyz, 1.0));
    var view_position = position_world_to_view(world_position.xyz);

    let radius = functions::compute_point_size(
        vertex,
        view_position,
        material.point_size,
        material.min_point_size,
        material.max_point_size
    );

    // Compute the offset to apply for creating a quad
    let offset = vertex.position.xy * radius;

    // Apply the offset to the view position and compute clip position
    let clip_position = position_view_to_clip(view_position + vec3<f32>(offset, 0.0));

    var out: types::VertexOutput;

    out.clip_position = clip_position;
    out.view_position = view_position;

    out.color = vertex.i_color;

#ifdef IS_OCTREE
#ifdef DEBUG_COLOR
    var debug_color = vec3<f32>(1.0, 1.0, 1.0);

    let max_relative_depth = functions::get_max_relative_depth(bindings::octree_entity, bindings::octree_node, bindings::visible_nodes, vertex.i_pos_size.xyz);
    let absolute_depth = u32(max_relative_depth) + bindings::octree_node.level;

    if absolute_depth == 0u {
        debug_color = vec3<f32>(1.0, 0.0, 0.0); // Rouge = problème !
    } else if absolute_depth == 1u {
        debug_color = vec3<f32>(1.0, 1.0, 0.0); // Jaune
    } else if absolute_depth == 2u {
        debug_color = vec3<f32>(0.0, 1.0, 0.0); // Vert
    } else {
        debug_color = vec3<f32>(0.0, 0.0, f32(absolute_depth) / 10.0); // Bleu = profond
    }

    out.color = vec4<f32>(debug_color, 1.0);
#endif // DEBUG_COLOR
#endif // IS_OCTREE

    out.uv = vertex.position.xy + vec2(0.5);
    out.log_depth = log2(-view_position.z);
    out.radius = radius;

	#ifdef HQ_DEPTH_PASS
		let original_depth = clip_position.w;
		let adjusted_depth = original_depth + 2.0 * radius;
		let adjust = adjusted_depth / original_depth;

        out.clip_position = position_view_to_clip(view_position * adjust + vec3<f32>(offset, 0.0));
	#endif

    return out;
}

struct FragmentOutput {
#ifdef DEPTH_PASS
    #ifdef USE_EDL
    @location(0) depth_texture: vec2<f32>,
    #else // USE EDL
    @location(0) depth_texture: f32,
    #endif // USE EDL
#else
    @location(0) color: vec4<f32>,
#endif
    @builtin(frag_depth) depth: f32,
}

@fragment
fn fragment(in: types::VertexOutput) -> FragmentOutput {
    let u = 2.0 * in.uv.x - 1.0;
    let v = 2.0 * in.uv.y - 1.0;
    let cc = u*u + v*v;
    if(cc > 1.0){
        discard;
    }

    // convert the color to linear RGB
    let color = functions::srgb_to_rgb_simple(in.color.xyz);

    var output: FragmentOutput;

    output.depth = in.clip_position.z;

#ifdef DEPTH_PASS
    #ifdef USE_EDL
        output.depth_texture.r = in.clip_position.z;
        output.depth_texture.g = in.log_depth;
    #else // USE_EDL
        output.depth_texture = in.clip_position.z;
    #endif // USE_EDL

    #ifdef PARABOLOID_POINT_SHAPE
    let radius = in.radius;
    let wi = 0.0 - cc;
    var pos = in.view_position;

    pos.z += wi * radius;
    let linear_depth = -pos.z;
    let clip_pos = position_view_to_ndc(pos);
    let exp_depth = clip_pos.z * 2.0 - 1.0;

    output.depth = clip_pos.z;
    #endif
#else // DEPTH_PASS
    output.color = vec4(color, 1.0);

    #ifdef PARABOLOID_POINT_SHAPE
    let radius = in.radius;
    let wi = 0.0 - cc;
    var pos = in.view_position;

    pos.z += wi * radius;
    let linear_depth = -pos.z;
    let clip_pos = position_view_to_ndc(pos);
    let exp_depth = clip_pos.z * 2.0 - 1.0;

    output.depth = clip_pos.z;
    #endif

    #ifdef WEIGHTED_SPLATS
    let distance = sqrt(cc);
    var weight = max(0.0, 1.0 - distance);
    weight = pow(weight, 1.5);

    output.color = vec4(color * weight, weight);
    #endif

#endif // DEPTH_PASS

    return output;
}
