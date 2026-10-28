#import bevy_pbr::{
    prepass_bindings,
    skinning,
    morph,
    mesh_view_bindings::view,
    view_transformations::position_world_to_view,
}
#import bevy_pointcloud::{
    simple_material_types as material_types,
}

#ifdef PREPASS_FRAGMENT
    #import bevy_pbr::prepass_io::FragmentOutput
#endif


#import bevy_pointcloud::{
    prepass_io::{ShapeInput, InstanceInput, VertexOutput},
    functions,
    pointcloud_bindings::pointcloud,
    pointcloud_functions,
}

#ifdef DEFERRED_PREPASS
#import bevy_pbr::rgb9e5
#endif

#ifdef POINTCLOUD_VIEW_SETTINGS
@group(#{POINTCLOUD_BIND_GROUP}) @binding(3) var<uniform> view_settings: material_types::PointCloudViewSettings;
#endif // POINTCLOUD_VIEW_SETTINGS


@fragment
fn fragment(in: VertexOutput)
#ifdef PREPASS_FRAGMENT
    -> FragmentOutput
#endif // PREPASS_FRAGMENT
{
#ifdef SHAPE_UVS_A
    #ifdef SPLAT_RADIUS
        // Perfect circle
        let dist = distance(in.shape_uv, vec2<f32>(0.5, 0.5));

        if dist > pointcloud.radius {
            discard;
        }
    #endif // SPLAT_RADIUS
#endif // SHAPE_UVS_A

#ifdef POINTCLOUD_VIEW_SETTINGS
    for (var i: u32 = 0u; i < view_settings.nb_clipping_planes; i ++) {
        let clip_dist = dot(in.world_position.xyz, view_settings.clipping_planes[i].xyz) + view_settings.clipping_planes[i].w;
        if (clip_dist < 0.0) {
            discard;
        }
    }
#endif // POINTCLOUD_VIEW_SETTINGS


#ifdef PREPASS_FRAGMENT
    var out: FragmentOutput;
#endif // PREPASS_FRAGMENT

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

#ifdef PREPASS_FRAGMENT
    return out;
#endif // PREPASS_FRAGMENT
}
