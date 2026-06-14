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

#import bevy_pointcloud::materials::simple::binding as material_binding

@vertex
fn vertex(vertex: types::Vertex) -> types::VertexOutput {
    // Compute world & view position of the point instance (applying the bindings::world_from_local matrix)
    let world_position = mesh_position_local_to_world(bindings::world_from_local, vec4<f32>(vertex.i_pos_size.xyz, 1.0));
    var view_position = position_world_to_view(world_position.xyz);

    let radius = functions::compute_point_size(
        vertex,
        view_position,
        material_binding::material.point_size,
        material_binding::material.min_point_size,
        material_binding::material.max_point_size
    );

    // Compute the offset to apply for creating a quad
    let offset = vertex.position.xy * radius;

    // Apply the offset to the view position and compute clip position
    let clip_position = position_view_to_clip(view_position + vec3<f32>(offset, 0.0));

    var out: types::VertexOutput;

    out.clip_position = clip_position;
    out.view_position = view_position;
    out.color = vertex.i_color;
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
