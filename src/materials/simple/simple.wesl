import bevy_pbr::render::{mesh_view_bindings::view};

import package::{
    materials::simple::{
        simple_bindings as material_bindings,
        simple_functions as material_functions,
        simple_types as material_types
    },
    render::{
        functions,
        pointcloud_functions,
        forward_io::{InstanceInput, ShapeInput, VertexOutput},
        pointcloud_bindings::pointcloud
    }
};

@if(!POINTCLOUD_PASS && MATERIAL_OIT_ENABLED)
import bevy_core_pipeline::oit::draw::oit_draw;

@if(POINTCLOUD_VIEW_SETTINGS) {
    @group(constants::POINTCLOUD_BIND_GROUP) @binding(3)
    var<uniform> view_settings: material_types::PointCloudViewSettings;
}

struct FragmentOutput {
    @if(POINTCLOUD_PASS_DEPTH && RENDER_MODE_EDL)
    @location(0) depth_texture: vec2<f32>,
    @if(POINTCLOUD_PASS_DEPTH && !RENDER_MODE_EDL)
    @location(0) depth_texture: f32,
    @if(!POINTCLOUD_PASS_DEPTH)
    @location(0) color: vec4<f32>,
}

@fragment
fn fragment(vertex_output: VertexOutput) -> FragmentOutput {
    var in = vertex_output;
    var out: FragmentOutput;

    @if(SPLAT_RADIUS) {
        // Perfect circle
        let diff = in.shape_uv - vec2<f32>(0.5, 0.5);
        let dist_sq = dot(diff, diff);
        let radius_sq = pointcloud.radius * pointcloud.radius;

        if dist_sq > radius_sq {
            discard;
        }
    }

    @if(POINTCLOUD_VIEW_SETTINGS && CLIPPING_PLANES) {
        for(var i: u32 = 0u; i < view_settings.nb_clipping_planes; i++) {
            let clip_dist = dot(in.world_position.xyz, view_settings.clipping_planes[i].xyz)
                + view_settings.clipping_planes[i].w;
            if clip_dist < 0.0 {
                discard;
            }
        }
    }

    @if(POINTCLOUD_PASS_DEPTH) {
        @if(RENDER_MODE_EDL) {
            out.depth_texture.r = in.position.z;
            out.depth_texture.g = in.log_depth;
        } @else {
            out.depth_texture = in.position.z;
        }
    } @else {
        var base_color = material_bindings::material.base_color;

        @if(SIMPLE_MATERIAL_GRADIENT) {
            let projected_dist = dot(
                    in.world_position.xyz,
                    material_bindings::material.gradient_direction.xyz,
                );
            let range = material_bindings::material.gradient_end
                - material_bindings::material.gradient_start;
            var t = 0.0;
            if range > 0.0 {
                t = clamp(
                        (projected_dist - material_bindings::material.gradient_start) / range,
                        0.0,
                        1.0,
                    );
            }

            base_color = material_functions::evaluate_gradient(
                    material_bindings::material.base_color,
                    material_bindings::material.end_color,
                    material_bindings::material.color_stops,
                    t,
                );
        }

        @if(POINTCLOUD_NEEDS_UVS) {
            if (material_bindings::material.flags
                    & material_types::SIMPLE_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT) != 0u {
                // We determine which UV coordinates to use for the texture sampling
                var texture_uv = in.uv;

                // Apply 2D affine transformation on global/planar UV coordinates
                @if(SIMPLE_MATERIAL_HAS_UV_TRANSFORM) {
                    let uv_rot_scale = mat2x2<f32>(
                            material_bindings::material.uv_transform_a.xy,
                            material_bindings::material.uv_transform_a.zw,
                        );
                    let uv_translation = material_bindings::material.uv_transform_b.xy;

                    texture_uv = (uv_rot_scale * texture_uv) + uv_translation;
                }

                let texture_color = textureSample(
                        material_bindings::base_color_texture,
                        material_bindings::base_color_sampler,
                        texture_uv,
                    );

                base_color *= texture_color;
            }
        }

        // TODO: do not compute on the single pass path
        @if(WEIGHTED_SPLATS_ENABLED) {
            @if(SPLAT_RADIUS)
            let dist = sqrt(dist_sq);
            @else
            let dist = distance(in.shape_uv, vec2<f32>(0.5, 0.5));

            var weight = max(0.0, 1.0 - dist * 2.0);
            weight = pow(weight, 1.5);
        } @else {
            let weight = 1.0;
        }

        @if(VERTEX_COLORS) {
            let color = functions::srgb_to_rgb_simple(in.color.xyz * base_color.xyz);
            let alpha = in.color.a * base_color.a;
        } @else {
            let color = functions::srgb_to_rgb_simple(base_color.xyz);
            let alpha = base_color.a;
        }

        out.color = vec4(color, alpha * weight);

        @if(!POINTCLOUD_PASS && MATERIAL_OIT_ENABLED) {
            let alpha_mode = pbr_input.material.flags
                & material_types::SIMPLE_MATERIAL_FLAGS_ALPHA_MODE_RESERVED_BITS;
            if alpha_mode == material_types::SIMPLE_MATERIAL_FLAGS_ALPHA_MODE_BLEND {
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
    } // POINTCLOUD_PASS_DEPTH

    return out;
}
