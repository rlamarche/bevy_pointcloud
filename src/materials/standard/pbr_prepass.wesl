import bevy_pbr::render::{
    pbr_bindings,
    pbr_functions,
    pbr_prepass_functions,
    pbr_types,
    prepass_io,
    mesh_view_bindings::view,
    pbr_bindings::material,
    pbr_functions::SampleBias
};

import bevy_render::bindless::{bindless_samplers_filtering, bindless_textures_2d};

// ============================================================================
// BEGIN CUSTOM PATCH: [PointCloudPlugin] - Point size and depth attenuation
// `VertexOutput` is no more imported in each case because imported below.
// ============================================================================
import bevy_pbr::prepass::io::{FragmentOutput, VertexOutput as PbrVertexOutput};

import package::render::prepass::prepass_io::VertexOutput;
import package::{
    materials::simple::{simple_types as material_types},
    render::pointcloud_bindings::pointcloud
};

@if(POINTCLOUD_VIEW_SETTINGS)
@group(constants::POINTCLOUD_BIND_GROUP) @binding(3)
var<uniform> view_settings: material_types::PointCloudViewSettings;

// ============================================================================
// END CUSTOM PATCH: [PointCloudPlugin]
// ============================================================================

@if(PREPASS_FRAGMENT) {
    @fragment
    fn fragment(
        vertex_output: VertexOutput,
        @builtin(front_facing) is_front: bool,
    ) -> prepass_io::FragmentOutput {
        @if(SPLAT_RADIUS) {
            // Perfect circle
            let diff = vertex_output.shape_uv - vec2<f32>(0.5, 0.5);
            let dist_sq = dot(diff, diff);
            let radius_sq = pointcloud.radius * pointcloud.radius;

            if dist_sq > radius_sq {
                discard;
            }
        }

        @if(POINTCLOUD_VIEW_SETTINGS && CLIPPING_PLANES) {
            for(var i: u32 = 0u; i < view_settings.nb_clipping_planes; i++) {
                let clip_dist =
                    dot(vertex_output.world_position.xyz, view_settings.clipping_planes[i].xyz)
                    + view_settings.clipping_planes[i].w;
                if clip_dist < 0.0 {
                    discard;
                }
            }
        }

        var in: PbrVertexOutput;

        in.position = vertex_output.position;

        @if(VERTEX_UVS_A) {
            in.uv = vertex_output.uv;
        }

        @if(VERTEX_UVS_B) {
            in.uv_b = vertex_output.uv_b;
        }

        @if(NORMAL_PREPASS_OR_DEFERRED_PREPASS) {
            in.world_normal = vertex_output.world_normal;

            @if(VERTEX_TANGENTS) {
                in.world_tangent = vertex_output.world_tangent;
            }
        }

        in.world_position = vertex_output.world_position;

        @if(MOTION_VECTOR_PREPASS) {
            in.previous_world_position = vertex_output.previous_world_position;
        }

        @if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
            in.unclipped_depth = vertex_output.unclipped_depth;
        }

        @if(VERTEX_OUTPUT_INSTANCE_INDEX) {
            in.instance_index = vertex_output.instance_index;
        }

        @if(VERTEX_COLORS) {
            in.color = vertex_output.color;
        }

        @if(VISIBILITY_RANGE_DITHER) {
            in.visibility_range_dither = vertex_output.visibility_range_dither;
        }

        let flags = pbr_bindings::material.flags;
        let uv_transform = pbr_bindings::material.uv_transform;

        // If we're in the crossfade section of a visibility range, conditionally
        // discard the fragment according to the visibility pattern.
        @if(VISIBILITY_RANGE_DITHER) {
            visibility_range_dither(in.position, in.visibility_range_dither);
        }

        pbr_prepass_functions::prepass_sample_color_and_alpha_discard(in);

        var out: prepass_io::FragmentOutput;

        @if(UNCLIPPED_DEPTH_ORTHO_EMULATION)
        out.frag_depth = in.unclipped_depth;

        // NOTE: Unlit bit not set means == 0 is true, so the true case is if lit
        @if(NORMAL_PREPASS)
        if (flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
            let double_sided = (flags & pbr_types::STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;
            @if(MESHLET_MESH_MATERIAL_PASS)
            let mesh_flags = in.mesh_flags;
            @else
            let mesh_flags = mesh[in.instance_index].flags;
            let is_logically_front = pbr_functions::winding_corrected_front_facing(
                    mesh_flags,
                    is_front,
                );

            let world_normal = pbr_functions::prepare_world_normal(
                    in.world_normal,
                    double_sided,
                    is_logically_front,
                );

            var normal = world_normal;

            @if(VERTEX_UVS) {
                @if(VERTEX_TANGENTS) {
                    @if(STANDARD_MATERIAL_NORMAL_MAP) {
                        // TODO: Transforming UVs mean we need to apply derivative chain rule for meshlet mesh material pass
                        @if(STANDARD_MATERIAL_NORMAL_MAP_UV_B)
                        let uv = (uv_transform * vec3(in.uv_b, 1.0)).xy;
                        @else
                        let uv = (uv_transform * vec3(in.uv, 1.0)).xy;

                        // Fill in the sample bias so we can sample from textures.
                        var bias: SampleBias;
                        @if(MESHLET_MESH_MATERIAL_PASS) {
                            // Apply mip bias by scaling the gradients used by textureSampleGrad.
                            let mip_scale = exp2(view.mip_bias);
                            bias.ddx_uv = in.ddx_uv * mip_scale;
                            bias.ddy_uv = in.ddy_uv * mip_scale;
                        }
                        @if(!MESHLET_MESH_MATERIAL_PASS)
                        bias.mip_bias = view.mip_bias;

                        @if(BINDLESS)
                        let texture = bindless_textures_2d[
                                material_indices[slot].normal_map_texture
                            ];
                        @else
                        let texture = pbr_bindings::normal_map_texture;
                        @if(BINDLESS)
                        let sampler_ = bindless_samplers_filtering[
                                material_indices[slot].normal_map_sampler
                            ];
                        @else
                        let sampler_ = pbr_bindings::normal_map_sampler;
                        @if(MESHLET_MESH_MATERIAL_PASS)
                        let Nt = textureSampleGrad(texture, sampler_, uv, bias.ddx_uv, bias.ddy_uv)
                                .rgb;
                        @if(!MESHLET_MESH_MATERIAL_PASS)
                        let Nt = textureSampleBias(texture, sampler_, uv, bias.mip_bias).rgb;
                        let TBN = pbr_functions::calculate_tbn_mikktspace(normal, in.world_tangent);

                        normal = pbr_functions::apply_normal_mapping(
                                flags,
                                TBN,
                                double_sided,
                                is_logically_front,
                                Nt,
                            );
                    }
                }
            }

            out.normal = vec4(normal * 0.5 + vec3(0.5), 1.0);
        } else {
            out.normal = vec4(in.world_normal * 0.5 + vec3(0.5), 1.0);
        }

        @if(MOTION_VECTOR_PREPASS && MESHLET_MESH_MATERIAL_PASS)
        out.motion_vector = in.motion_vector;
        @if(MOTION_VECTOR_PREPASS && !MESHLET_MESH_MATERIAL_PASS)
        out.motion_vector = pbr_prepass_functions::calculate_motion_vector(
                in.world_position,
                in.previous_world_position,
            );

        return out;
    }
} // PREPASS_FRAGMENT
@else {
    @fragment
    fn fragment(vertex_output: VertexOutput, @builtin(front_facing) is_front: bool) {
        @if(SPLAT_RADIUS) {
            // Perfect circle
            let diff = vertex_output.shape_uv - vec2<f32>(0.5, 0.5);
            let dist_sq = dot(diff, diff);
            let radius_sq = pointcloud.radius * pointcloud.radius;

            if dist_sq > radius_sq {
                discard;
            }
        }

        @if(POINTCLOUD_VIEW_SETTINGS && CLIPPING_PLANES) {
            for(var i: u32 = 0u; i < view_settings.nb_clipping_planes; i++) {
                let clip_dist =
                    dot(vertex_output.world_position.xyz, view_settings.clipping_planes[i].xyz)
                    + view_settings.clipping_planes[i].w;
                if clip_dist < 0.0 {
                    discard;
                }
            }
        }

        var in: PbrVertexOutput;

        in.position = vertex_output.position;

        @if(VERTEX_UVS_A) {
            in.uv = vertex_output.uv;
        }

        @if(VERTEX_UVS_B) {
            in.uv_b = vertex_output.uv_b;
        }

        @if(NORMAL_PREPASS_OR_DEFERRED_PREPASS) {
            in.world_normal = vertex_output.world_normal;

            @if(VERTEX_TANGENTS) {
                in.world_tangent = vertex_output.world_tangent;
            }
        }

        in.world_position = vertex_output.world_position;

        @if(MOTION_VECTOR_PREPASS) {
            in.previous_world_position = vertex_output.previous_world_position;
        }

        @if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
            in.unclipped_depth = vertex_output.unclipped_depth;
        }

        @if(VERTEX_OUTPUT_INSTANCE_INDEX) {
            in.instance_index = vertex_output.instance_index;
        }

        @if(VERTEX_COLORS) {
            in.color = vertex_output.color;
        }

        @if(VISIBILITY_RANGE_DITHER) {
            in.visibility_range_dither = vertex_output.visibility_range_dither;
        }

        pbr_prepass_functions::prepass_sample_color_and_alpha_discard(in);
    }
}
