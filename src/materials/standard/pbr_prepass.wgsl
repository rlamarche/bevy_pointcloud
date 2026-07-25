#ifdef PREPASS_FRAGMENT

#import bevy_pbr::{
    pbr_types,
    pbr_functions::alpha_discard,
    pbr_fragment::pbr_input_from_standard_material,
    decal::clustered::apply_decals,
}

#endif


// ============================================================================
// BEGIN CUSTOM PATCH: [PointCloudPlugin] - Point size and depth attenuation
// `VertexOutput` is no more imported in each case because imported below.
// ============================================================================
#import bevy_pbr::{
    prepass_io::{VertexOutput as PbrVertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}

#import bevy_pointcloud::prepass_io::VertexOutput
#import bevy_pointcloud::pointcloud_bindings::pointcloud

// ============================================================================
// END CUSTOM PATCH: [PointCloudPlugin]
// ============================================================================

#ifdef VISIBILITY_RANGE_DITHER
#import bevy_pbr::pbr_functions::visibility_range_dither;
#endif

#ifdef MESHLET_MESH_MATERIAL_PASS
#import bevy_pbr::meshlet_visibility_buffer_resolve::resolve_vertex_output
#endif

#ifdef OIT_ENABLED
#import bevy_core_pipeline::oit::oit_draw
#endif // OIT_ENABLED

#ifdef FORWARD_DECAL
#import bevy_pbr::decal::forward::get_forward_decal_info
#endif

@fragment
fn fragment(
#ifdef MESHLET_MESH_MATERIAL_PASS
    @builtin(position) frag_coord: vec4<f32>,
#else
    vertex_output: VertexOutput,
    @builtin(front_facing) is_front: bool,
#endif
)
#ifdef PREPASS_FRAGMENT
-> FragmentOutput
#endif // PREPASS_FRAGMENT
{
#ifdef MESHLET_MESH_MATERIAL_PASS
    let vertex_output = resolve_vertex_output(frag_coord);
    let is_front = true;
#endif

#ifdef SHAPE_UVS_A
    #ifdef SPLAT_RADIUS
        // Perfect circle
        let dist = distance(vertex_output.shape_uv, vec2<f32>(0.5, 0.5));

        if dist > pointcloud.radius {
            discard;
        }
    #endif // SIMPLE_MATERIAL_SHAPE_RADIUS
#endif // SHAPE_UVS_A

    var in: PbrVertexOutput;

#ifdef PREPASS_FRAGMENT
    in.position                    = vertex_output.position;

    #ifdef VERTEX_UVS_A
        in.uv                      = vertex_output.uv;
    #endif // VERTEX_UVS_A

    #ifdef VERTEX_UVS_B
        in.uv_b                    = vertex_output.uv_b;
    #endif // VERTEX_UVS_B

    #ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
        in.world_normal            = vertex_output.world_normal;

        #ifdef VERTEX_TANGENTS
            in.world_tangent       = vertex_output.world_tangent;
        #endif // VERTEX_TANGENTS
    #endif // NORMAL_PREPASS_OR_DEFERRED_PREPASS

    in.world_position              = vertex_output.world_position;
    #ifdef MOTION_VECTOR_PREPASS
        in.previous_world_position = vertex_output.previous_world_position;
    #endif

    #ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
        in.unclipped_depth         = vertex_output.unclipped_depth;
    #endif // UNCLIPPED_DEPTH_ORTHO_EMULATION

    #ifdef VERTEX_OUTPUT_INSTANCE_INDEX
        in.instance_index          = vertex_output.instance_index;
    #endif // VERTEX_OUTPUT_INSTANCE_INDEX

    #ifdef VERTEX_COLORS
        in.color                   = vertex_output.color;
    #endif // VERTEX_COLORS

    #ifdef VISIBILITY_RANGE_DITHER
        in.visibility_range_dither = vertex_output.visibility_range_dither;
    #endif // VISIBILITY_RANGE_DITHER

    // If we're in the crossfade section of a visibility range, conditionally
    // discard the fragment according to the visibility pattern.
#ifdef VISIBILITY_RANGE_DITHER
    visibility_range_dither(in.position, in.visibility_range_dither);
#endif

#ifdef FORWARD_DECAL
    let forward_decal_info = get_forward_decal_info(in);
    in.world_position = forward_decal_info.world_position;
    in.uv = forward_decal_info.uv;
#endif

    // generate a PbrInput struct from the StandardMaterial bindings
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // alpha discard
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

    // clustered decals
    apply_decals(&pbr_input);

#ifdef PREPASS_PIPELINE
    // write the gbuffer, lighting pass id, and optionally normal and motion_vector textures
    let out = deferred_output(in, pbr_input);
#else
    // in forward mode, we calculate the lit color immediately, and then apply some post-lighting effects here.
    // in deferred mode the lit color and these effects will be calculated in the deferred lighting shader
    var out: FragmentOutput;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }

    // apply in-shader post processing (fog, alpha-premultiply, and also tonemapping, debanding if the camera is non-hdr)
    // note this does not include fullscreen postprocessing effects like bloom.
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

#ifdef OIT_ENABLED
    let alpha_mode = pbr_input.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_RESERVED_BITS;
    if alpha_mode != pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_OPAQUE {
        // The fragments will only be drawn during the oit resolve pass.
        oit_draw(in.position, out.color);
        discard;
    }
#endif // OIT_ENABLED

#ifdef FORWARD_DECAL
        out.color.a = min(forward_decal_info.alpha, out.color.a);
#endif

    return out;
#endif // PREPASS_FRAGMENT
}
