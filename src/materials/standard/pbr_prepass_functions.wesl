import bevy_render::bindless::{bindless_samplers_filtering, bindless_textures_2d};

import bevy_pbr::{
    prepass::bindings::previous_view_uniforms,
    prepass::io::VertexOutput,
    render::{pbr_bindings, pbr_types, mesh_bindings::mesh, mesh_view_bindings::view}
};

import package::{
    materials::simple::{simple_types as material_types},
    render::pointcloud_bindings::pointcloud
};

@if(BINDLESS)
import bevy_pbr::render::pbr_bindings::material_indices;

/// Cutoff used for the premultiplied alpha modes BLEND, ADD, and ALPHA_TO_COVERAGE.
const PREMULTIPLIED_ALPHA_CUTOFF = 0.05;

/// Reusable alpha discard logic suitable for use by custom materials in the prepass.
/// We can use a simplified version of alpha_discard() here since we only need to handle the alpha_cutoff
fn prepass_alpha_discard(material_flags: u32, alpha_cutoff: f32, output_color: vec4<f32>) {
    let alpha_mode = material_flags & pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_RESERVED_BITS;
    if alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_MASK {
        if output_color.a < alpha_cutoff {
            discard;
        }
    } else if alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_BLEND
        || alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_ADD
        || alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_ALPHA_TO_COVERAGE {
        if output_color.a < PREMULTIPLIED_ALPHA_CUTOFF {
            discard;
        }
    } else if alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_PREMULTIPLIED {
        if all(output_color < vec4(PREMULTIPLIED_ALPHA_CUTOFF)) {
            discard;
        }
    }
}

/// Samples the StandardMaterial's base color (including the base color texture, if any) and then
/// invokes `prepass_alpha_discard` with the material's flags and alpha cutoff.
fn prepass_sample_color_and_alpha_discard(in: VertexOutput) {
    @if(MAY_DISCARD) {
        @if(BINDLESS) {
            let slot = pointcloud.material_bind_group_slot & 0xffffu;
            var output_color: vec4<f32> = pbr_bindings::material_array[
                    material_indices[slot].material
                ].base_color;
            let flags = pbr_bindings::material_array[material_indices[slot].material].flags;
            let alpha_cutoff = pbr_bindings::material_array[material_indices[slot].material]
                    .alpha_cutoff;
        } @else {
            var output_color: vec4<f32> = pbr_bindings::material.base_color;
            let flags = pbr_bindings::material.flags;
            let alpha_cutoff = pbr_bindings::material.alpha_cutoff;
        }

        @if(VERTEX_UVS) {
            @if(STANDARD_MATERIAL_BASE_COLOR_UV_B)
            var uv = in.uv_b;
            @else
            // STANDARD_MATERIAL_BASE_COLOR_UV_B
            var uv = in.uv;

            @if(BINDLESS)
            let uv_transform = pbr_bindings::material_array[material_indices[slot].material]
                    .uv_transform;
            @else
            let uv_transform = pbr_bindings::material.uv_transform;

            uv = (uv_transform * vec3(uv, 1.0)).xy;
            if (flags & pbr_types::STANDARD_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT) != 0u {
                @if(BINDLESS)
                let texture = bindless_textures_2d[material_indices[slot].base_color_texture];
                @else
                let texture = pbr_bindings::base_color_texture;
                @if(BINDLESS)
                let sampler_ = bindless_samplers_filtering[
                        material_indices[slot].base_color_sampler
                    ];
                @else
                let sampler_ = pbr_bindings::base_color_sampler;
                output_color = output_color
                    * textureSampleBias(texture, sampler_, uv, view.mip_bias);
            }
        }

        prepass_alpha_discard(flags, alpha_cutoff, output_color);
    }
}
