import bevy_pbr::{
    decal::clustered::apply_decals,
    render::{
        pbr_types,
        pbr_fragment::pbr_input_from_standard_material,
        pbr_functions::alpha_discard
    }
};

import bevy_pbr::render::pbr_bindings;

// ============================================================================
// BEGIN CUSTOM PATCH: [PointCloudPlugin] - Point size and depth attenuation
// `VertexOutput` is no more imported in each case because imported below.
// ============================================================================

@if(PREPASS_PIPELINE)
import bevy_pbr::{
    deferred::functions::deferred_output,
    prepass::io::{FragmentOutput, VertexOutput as PbrVertexOutput}
};

@if(!PREPASS_PIPELINE)
import bevy_pbr::render::{
    forward_io::{VertexOutput as PbrVertexOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT
};

@if(PREPASS_PIPELINE)
import package::render::prepass::prepass_io::VertexOutput;
@else
import package::render::forward_io::VertexOutput;

import package::{
    materials::simple::simple_types as material_types,
    render::{functions, pointcloud_bindings::pointcloud}
};

// ============================================================================
// END CUSTOM PATCH: [PointCloudPlugin]
// ============================================================================

@if(VISIBILITY_RANGE_DITHER)
import bevy_pbr::render::pbr_functions::visibility_range_dither;

@if(MESHLET_MESH_MATERIAL_PASS)
import bevy_pbr::meshlet::visibility_buffer_resolve::resolve_vertex_output;

@if(!POINTCLOUD_PASS && OIT_ENABLED)
import bevy_core_pipeline::oit::draw::oit_draw;

@if(FORWARD_DECAL)
import bevy_pbr::decal::get_forward_decal_info;

@if(POINTCLOUD_VIEW_SETTINGS)
@group(constants::POINTCLOUD_BIND_GROUP) @binding(3)
var<uniform> view_settings: material_types::PointCloudViewSettings;

@if(!PREPASS_PIPELINE)
struct FragmentOutput {
    @if(POINTCLOUD_PASS_DEPTH && RENDER_MODE_EDL)
    @location(0) depth_texture: vec2<f32>,
    @if(POINTCLOUD_PASS_DEPTH && !RENDER_MODE_EDL)
    @location(0) depth_texture: f32,
    @if(!POINTCLOUD_PASS_DEPTH)
    @location(0) color: vec4<f32>,
    // @builtin(frag_depth) depth: f32,
}

fn fragment_body(vertex_output: VertexOutput, is_front: bool) -> FragmentOutput {
    var dist_sq: f32 = 0.0;

    @if(SPLAT_RADIUS) {
        // Perfect circle
        let diff = vertex_output.shape_uv - vec2<f32>(0.5, 0.5);
        dist_sq = dot(diff, diff);
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

    var out: FragmentOutput;

    @if(POINTCLOUD_PASS_DEPTH) {
        @if(RENDER_MODE_EDL) {
            out.depth_texture.r = vertex_output.position.z;
            out.depth_texture.g = vertex_output.log_depth;
        } @else {
            out.depth_texture = vertex_output.position.z;
        }
    } @else {
        var in: PbrVertexOutput;

        in.position = vertex_output.position;
        in.world_position = vertex_output.world_position;
        in.world_normal = vertex_output.world_normal;

        @if(VERTEX_UVS_A) {
            in.uv = vertex_output.uv;
        }
        @if(VERTEX_UVS_B) {
            in.uv_b = vertex_output.uv_b;
        }
        @if(VERTEX_TANGENTS) {
            in.world_tangent = vertex_output.world_tangent;
        }
        @if(VERTEX_COLORS) {
            in.color = vec4<f32>(
                    functions::srgb_to_rgb_simple(vertex_output.color.xyz),
                    vertex_output.color.a,
                );
        }
        @if(VERTEX_OUTPUT_INSTANCE_INDEX) {
            in.instance_index = vertex_output.instance_index;
        }
        @if(VISIBILITY_RANGE_DITHER) {
            in.visibility_range_dither = vertex_output.visibility_range_dither;
        }

        // If we're in the crossfade section of a visibility range, conditionally
        // discard the fragment according to the visibility pattern.
        @if(VISIBILITY_RANGE_DITHER) {
            visibility_range_dither(in.position, in.visibility_range_dither);
        }

        @if(FORWARD_DECAL) {
            let forward_decal_info = get_forward_decal_info(in);
            in.world_position = forward_decal_info.world_position;
            in.uv = forward_decal_info.uv;
        }

        // generate a PbrInput struct from the StandardMaterial bindings
        var pbr_input = pbr_input_from_standard_material(in, is_front);

        // alpha discard
        pbr_input.material.base_color = alpha_discard(
                pbr_input.material.flags,
                pbr_input.material.alpha_cutoff,
                pbr_input.material.base_color,
            );

        // clustered decals
        apply_decals(&pbr_input);

        @if(PREPASS_PIPELINE) {
            // write the gbuffer, lighting pass id, and optionally normal and motion_vector textures
            out = deferred_output(in, pbr_input);
        } @else {
            // in forward mode, we calculate the lit color immediately, and then apply some post-lighting effects here.
            // in deferred mode the lit color and these effects will be calculated in the deferred lighting shader
            if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
                out.color = apply_pbr_lighting(pbr_input);
            } else {
                out.color = pbr_input.material.base_color;
            }

            // apply in-shader post processing (fog, alpha-premultiply, and also tonemapping, debanding if the camera is non-hdr)
            // note this does not include fullscreen postprocessing effects like bloom.
            out.color = main_pass_post_lighting_processing(pbr_input, out.color);
        }

        @if(!POINTCLOUD_PASS && MATERIAL_OIT_ENABLED) {
            let alpha_mode = pbr_input.material.flags
                & pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_RESERVED_BITS;
            if alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_BLEND {
                // The fragments will only be drawn during the oit resolve pass.
                oit_draw(in.position, vec4(out.color.rgb * out.color.a, out.color.a));
                discard;
            }
            // Both `Premultiplied` and `Add` colors are premultiplied in `premultiply_alpha()`
            if alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_PREMULTIPLIED
                || alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_ADD {
                // The fragments will only be drawn during the oit resolve pass.
                oit_draw(in.position, out.color);
                discard;
            }
        }

        @if(FORWARD_DECAL) {
            out.color.a = min(forward_decal_info.alpha, out.color.a);
        }

        // if ((pbr_bindings::material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT) != 0u) {
        //     let texture_color = textureSample(pbr_bindings::base_color_texture, pbr_bindings::base_color_sampler, in.uv);
        //     #ifdef VERTEX_UVS
        //     out.color = texture_color;
        //     #endif
        // }

        // out.color = pbr_bindings::material.base_color;
        // out.color = vec4<f32>(in.uv.x, in.uv.y, in.uv.x, 1.0);

        // TODO: do not compute on the single pass path
        @if(!PREPASS_PIPELINE && WEIGHTED_SPLATS_ENABLED) {
            var dist: f32;
            @if(SPLAT_RADIUS) {
                dist = sqrt(dist_sq);
            } @else {
                dist = distance(vertex_output.shape_uv, vec2<f32>(0.5, 0.5));
            }

            var weight = max(0.0, 1.0 - dist * 2.0);
            weight = pow(weight, 1.5);
            out.color.a = weight;
        }
    }

    return out;
}

@if(MESHLET_MESH_MATERIAL_PASS)
@fragment
fn fragment(@builtin(position) frag_coord: vec4<f32>) -> FragmentOutput {
    let vertex_output = resolve_vertex_output(frag_coord);
    return fragment_body(vertex_output, true);
}

@if(!MESHLET_MESH_MATERIAL_PASS)
@fragment
fn fragment(vertex_output: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    return fragment_body(vertex_output, is_front);
}
