#import bevy_render::view::position_view_to_world;
#import bevy_pbr::{
    prepass_bindings,
    mesh_bindings::mesh,
    mesh_functions,
    // prepass_io::{Vertex, VertexOutput, FragmentOutput},
    skinning,
    morph,
    morph::{morph_position, morph_normal, morph_tangent},
    mesh_view_bindings::view,
    view_transformations,
}

#import bevy_pointcloud::{
    prepass_io::{ShapeInput, InstanceInput, VertexOutput, FragmentOutput},
    functions,
    pointcloud_bindings::pointcloud,
    pointcloud_functions,
}

#ifdef DEFERRED_PREPASS
#import bevy_pbr::rgb9e5
#endif

@vertex
fn vertex(
    shape: ShapeInput,
    vertex: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    out.instance_index = 0;

    // We assume VERTEX_POSITIONS & SHAPE_POSITIONS are set.

    #ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
        #ifdef VERTEX_NORMALS
            out.world_normal = pointcloud_functions::mesh_normal_local_to_world(vertex.normal);
        #else
            out.world_normal = normalize(-view.world_from_view[2].xyz);
        #endif
    #endif


    let world_from_local = pointcloud_functions::get_world_from_local();

    // compute the world position of point coordinates
    let world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    var view_vertex_position: vec3<f32>;

    let view_position = view_transformations::position_world_to_view(world_position.xyz);

    var radius: f32 = 0.5;
    let radius_scale = functions::extract_max_scale(world_from_local);

    var world_vertex_position: vec3<f32>;

    #ifdef VERTEX_NORMALS
        let normal: vec3<f32> = vertex.normal;

        #ifdef VERTEX_TANGENTS
            world_vertex_position = functions::compute_world_vertex_position_oriented_with_tangent(
                world_position.xyz,
                normal,
                vertex.tangent.xyz,
                world_from_local,
                shape.position,
                radius
            );
        #else
            world_vertex_position = functions::compute_world_vertex_position_oriented(
                world_position.xyz,
                normal,
                world_from_local,
                shape.position,
                radius
            );
        #endif

        view_vertex_position = view_transformations::position_world_to_view(world_vertex_position);
    #else // case billboard
        view_vertex_position = functions::compute_view_billboard_vertex_position(
            view_position,
            shape.position,
            radius
        );

        world_vertex_position = position_view_to_world(view_vertex_position, view.world_from_view);
    #endif


    out.position = view_transformations::position_view_to_clip(view_vertex_position);
    out.world_position = vec4<f32>(world_vertex_position, 1.0);


#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0); // Clamp depth to avoid clipping
#endif // UNCLIPPED_DEPTH_ORTHO_EMULATION

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif // VERTEX_UVS_A

#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif // VERTEX_UVS_B

// #ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
// #ifdef VERTEX_NORMALS
// #ifdef SKINNED
//     out.world_normal = skinning::skin_normals(world_from_local, vertex.normal);
// #else // SKINNED
//     out.world_normal = mesh_functions::mesh_normal_local_to_world(
//         vertex.normal,
//         // Use vertex_no_morph.instance_index instead of vertex.instance_index to work around a wgpu dx12 bug.
//         // See https://github.com/gfx-rs/naga/issues/2416
//         vertex_no_morph.instance_index
//     );
// #endif // SKINNED
// #endif // VERTEX_NORMALS

// #ifdef VERTEX_TANGENTS
//     out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
//         world_from_local,
//         vertex.tangent,
//         // Use vertex_no_morph.instance_index instead of vertex.instance_index to work around a wgpu dx12 bug.
//         // See https://github.com/gfx-rs/naga/issues/2416
//         vertex_no_morph.instance_index
//     );
// #endif // VERTEX_TANGENTS
// #endif // NORMAL_PREPASS_OR_DEFERRED_PREPASS

#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif

#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_no_morph.instance_index, mesh_world_from_local[3]);
#endif  // VISIBILITY_RANGE_DITHER

    return out;
}

#ifdef PREPASS_FRAGMENT
@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

#ifdef NORMAL_PREPASS
    out.normal = vec4(in.world_normal * 0.5 + vec3(0.5), 1.0);
#endif

#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.frag_depth = in.unclipped_depth;
#endif // UNCLIPPED_DEPTH_ORTHO_EMULATION

#ifdef MOTION_VECTOR_PREPASS
    let clip_position_t = view.unjittered_clip_from_world * in.world_position;
    let clip_position = clip_position_t.xy / clip_position_t.w;
    let previous_clip_position_t = prepass_bindings::previous_view_uniforms.clip_from_world * in.previous_world_position;
    let previous_clip_position = previous_clip_position_t.xy / previous_clip_position_t.w;
    // These motion vectors are used as offsets to UV positions and are stored
    // in the range -1,1 to allow offsetting from the one corner to the
    // diagonally-opposite corner in UV coordinates, in either direction.
    // A difference between diagonally-opposite corners of clip space is in the
    // range -2,2, so this needs to be scaled by 0.5. And the V direction goes
    // down where clip space y goes up, so y needs to be flipped.
    out.motion_vector = (clip_position - previous_clip_position) * vec2(0.5, -0.5);
#endif // MOTION_VECTOR_PREPASS

#ifdef DEFERRED_PREPASS
    // There isn't any material info available for this default prepass shader so we are just writing 
    // emissive magenta out to the deferred gbuffer to be rendered by the first deferred lighting pass layer.
    // This is here so if the default prepass fragment is used for deferred magenta will be rendered, and also
    // as an example to show that a user could write to the deferred gbuffer if they were to start from this shader.
    out.deferred = vec4(0u, bevy_pbr::rgb9e5::vec3_to_rgb9e5_(vec3(1.0, 0.0, 1.0)), 0u, 0u);
    out.deferred_lighting_pass_id = 1u;
#endif

    return out;
}
#endif // PREPASS_FRAGMENT
