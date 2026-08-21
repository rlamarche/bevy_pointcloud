#import bevy_pbr::{
    pbr_prepass_functions,
    pbr_bindings,
    pbr_bindings::material,
    pbr_types,
    pbr_functions,
    pbr_functions::SampleBias,
    prepass_io,
    mesh_view_bindings::view,
}

#import bevy_render::bindless::{bindless_samplers_filtering, bindless_textures_2d}

// ============================================================================
// BEGIN CUSTOM PATCH: [PointCloudPlugin] - Point size and depth attenuation
// `VertexOutput` is no more imported in each case because imported below.
// ============================================================================
#import bevy_pbr::{
    prepass_io::{VertexOutput as PbrVertexOutput, FragmentOutput},
}

#import bevy_pointcloud::prepass_io::VertexOutput
#import bevy_pointcloud::pointcloud_bindings::pointcloud

// ============================================================================
// END CUSTOM PATCH: [PointCloudPlugin]
// ============================================================================


@fragment
fn fragment(
    vertex_output: VertexOutput,
    @builtin(front_facing) is_front: bool,
)
#ifdef PREPASS_FRAGMENT
-> prepass_io::FragmentOutput
#endif // PREPASS_FRAGMENT
{
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

    #ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
        in.unclipped_depth = vertex_output.unclipped_depth;
    #endif // UNCLIPPED_DEPTH_ORTHO_EMULATION

#ifdef PREPASS_FRAGMENT
    let flags = pbr_bindings::material.flags;
    let uv_transform = pbr_bindings::material.uv_transform;


    // If we're in the crossfade section of a visibility range, conditionally
    // discard the fragment according to the visibility pattern.
    #ifdef VISIBILITY_RANGE_DITHER
        visibility_range_dither(in.position, in.visibility_range_dither);
    #endif

    pbr_prepass_functions::prepass_alpha_discard(in);

    var out: prepass_io::FragmentOutput;

    #ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
        out.frag_depth = in.unclipped_depth;
    #endif // UNCLIPPED_DEPTH_ORTHO_EMULATION

    #ifdef NORMAL_PREPASS
        // NOTE: Unlit bit not set means == 0 is true, so the true case is if lit
        if (flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
            let double_sided = (flags & pbr_types::STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;

            let world_normal = pbr_functions::prepare_world_normal(
                in.world_normal,
                double_sided,
                is_front,
            );

            var normal = world_normal;

    #ifdef VERTEX_UVS
    #ifdef VERTEX_TANGENTS
    #ifdef STANDARD_MATERIAL_NORMAL_MAP

    // TODO: Transforming UVs mean we need to apply derivative chain rule for meshlet mesh material pass
    #ifdef STANDARD_MATERIAL_NORMAL_MAP_UV_B
            let uv = (uv_transform * vec3(in.uv_b, 1.0)).xy;
    #else
            let uv = (uv_transform * vec3(in.uv, 1.0)).xy;
    #endif

            // Fill in the sample bias so we can sample from textures.
            var bias: SampleBias;
    #ifdef MESHLET_MESH_MATERIAL_PASS
            bias.ddx_uv = in.ddx_uv;
            bias.ddy_uv = in.ddy_uv;
    #else   // MESHLET_MESH_MATERIAL_PASS
            bias.mip_bias = view.mip_bias;
    #endif  // MESHLET_MESH_MATERIAL_PASS

            let Nt =
    #ifdef MESHLET_MESH_MATERIAL_PASS
                textureSampleGrad(
    #else   // MESHLET_MESH_MATERIAL_PASS
                textureSampleBias(
    #endif  // MESHLET_MESH_MATERIAL_PASS
    #ifdef BINDLESS
                    bindless_textures_2d[material_indices[slot].normal_map_texture],
                    bindless_samplers_filtering[material_indices[slot].normal_map_sampler],
    #else   // BINDLESS
                    pbr_bindings::normal_map_texture,
                    pbr_bindings::normal_map_sampler,
    #endif  // BINDLESS
                    uv,
    #ifdef MESHLET_MESH_MATERIAL_PASS
                    bias.ddx_uv,
                    bias.ddy_uv,
    #else   // MESHLET_MESH_MATERIAL_PASS
                    bias.mip_bias,
    #endif  // MESHLET_MESH_MATERIAL_PASS
                ).rgb;
            let TBN = pbr_functions::calculate_tbn_mikktspace(normal, in.world_tangent);

            normal = pbr_functions::apply_normal_mapping(
                flags,
                TBN,
                double_sided,
                is_front,
                Nt,
            );

    #endif  // STANDARD_MATERIAL_NORMAL_MAP
    #endif  // VERTEX_TANGENTS
    #endif  // VERTEX_UVS

            out.normal = vec4(normal * 0.5 + vec3(0.5), 1.0);
        } else {
            out.normal = vec4(in.world_normal * 0.5 + vec3(0.5), 1.0);
        }
    #endif // NORMAL_PREPASS

    #ifdef MOTION_VECTOR_PREPASS
    #ifdef MESHLET_MESH_MATERIAL_PASS
        out.motion_vector = in.motion_vector;
    #else
        out.motion_vector = pbr_prepass_functions::calculate_motion_vector(in.world_position, in.previous_world_position);
    #endif
    #endif

    return out;
#else // PREPASS_FRAGMENT
    pbr_prepass_functions::prepass_alpha_discard(in);
#endif // PREPASS_FRAGMENT
}
