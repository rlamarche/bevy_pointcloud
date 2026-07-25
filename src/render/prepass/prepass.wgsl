#import bevy_render::view::position_view_to_world;
#import bevy_pbr::{
    prepass_bindings,
    mesh_bindings::mesh,
    mesh_functions,
    skinning,
    morph,
    mesh_view_bindings::view,
    view_transformations::position_world_to_view,
    prepass_io::FragmentOutput,
}

#import bevy_pointcloud::{
    prepass_io::{ShapeInput, InstanceInput, VertexOutput},
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

    let world_from_local = pointcloud_functions::get_world_from_local();

    // TODO remove world_from_local from PointCloudUniform and use this above
    // let world_from_local = mesh_functions::get_world_from_local(out.instance_index);

    let point_world_position =
        mesh_functions::mesh_position_local_to_world(
            world_from_local,
            vec4<f32>(vertex.position, 1.0)
        ).xyz;

    let point_view_position = position_world_to_view(point_world_position);

    let radius = pointcloud_functions::compute_point_radius(
        world_from_local,
        point_view_position
    );

    // Prepare normal (fallback to default)
    #ifdef VERTEX_NORMALS
        let normal = vertex.normal;
    #else
        let normal = pointcloud.default_normal.xyz;
    #endif

    // Prepare tangent
    #ifdef VERTEX_TANGENTS
        let tangent = vertex.tangent.xyz;
    #else
        let tangent = vec3<f32>(0.0);
    #endif

    // Compute vertex positions
    let pos = pointcloud_functions::compute_point_vertex_positions(
        point_world_position,
        world_from_local,
        shape.position,
        radius,
        normal,
        tangent,
    );

    out.position = pos.clip_position;
    out.world_position = vec4<f32>(pos.world_position, 1.0);

#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0); // Clamp depth to avoid clipping
#endif // UNCLIPPED_DEPTH_ORTHO_EMULATION

#ifdef INSTANCE_UVS_A
    out.uv = vertex.uv;
#endif // VERTEX_UVS_A

#ifdef INSTANCE_UVS_B
    out.uv_b = vertex.uv_b;
#endif // VERTEX_UVS_B

#ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(normal, out.instance_index);
#endif // VERTEX_NORMALS

#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        vertex.tangent,
        out.instance_index,
    );
#endif // VERTEX_TANGENTS
#endif // NORMAL_PREPASS_OR_DEFERRED_PREPASS


#ifdef VISIBILITY_RANGE_DITHER
    let mesh_world_from_local = mesh_functions::get_world_from_local(out.instance_index);

    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_no_morph.instance_index, mesh_world_from_local[3]);
#endif  // VISIBILITY_RANGE_DITHER

    #ifdef SHAPE_UVS_A
        out.shape_uv = shape.uv;
    #endif // SHAPE_UVS_A

    return out;
}

#ifdef PREPASS_FRAGMENT
@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
#ifdef SHAPE_UVS_A
    #ifdef SPLAT_RADIUS
        // Perfect circle
        let dist = distance(in.shape_uv, vec2<f32>(0.5, 0.5));

        if dist > pointcloud.radius {
            discard;
        }
    #endif // SPLAT_RADIUS
#endif // SHAPE_UVS_A

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
