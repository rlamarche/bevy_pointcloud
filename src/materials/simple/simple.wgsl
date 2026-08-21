#import bevy_pbr::{
    view_transformations::position_world_to_view,
    forward_io::FragmentOutput,
}

#import bevy_pointcloud::{
    forward_io::{ShapeInput, InstanceInput, VertexOutput},
    functions,
    pointcloud_bindings::pointcloud,
    pointcloud_functions,
    simple_material_types as material_types,
    simple_material_functions as material_functions,
    simple_material_bindings as material_bindings,
}


@fragment
fn fragment(
    vertex_output: VertexOutput,
) -> FragmentOutput {
    var in = vertex_output;
    var out: FragmentOutput;

    #ifdef SHAPE_UVS_A
        #ifdef SPLAT_RADIUS
        // Perfect circle
        let dist = distance(in.shape_uv, vec2<f32>(0.5, 0.5));

        if dist > pointcloud.radius {
            discard;
        }
        #endif // SPLAT_RADIUS
    #endif // SHAPE_UVS_A

    var base_color = material_bindings::material.base_color;

    #ifdef SIMPLE_MATERIAL_GRADIENT

        let projected_dist = dot(in.world_position.xyz, material_bindings::material.gradient_direction.xyz);
        let range = material_bindings::material.gradient_end - material_bindings::material.gradient_start;
        var t = 0.0;
        if range > 0.0 {
            t = clamp((projected_dist - material_bindings::material.gradient_start) / range, 0.0, 1.0);
        }

        base_color = material_functions::evaluate_gradient(
            material_bindings::material.base_color,
            material_bindings::material.end_color,
            material_bindings::material.color_stops,
        t);
    #endif // SIMPLE_MATERIAL_GRADIENT

    if ((material_bindings::material.flags & material_types::SIMPLE_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT) != 0u) {


        // We determine which UV coordinates to use for the texture sampling
        var texture_uv = in.uv; //vec2<f32>(0.5, 0.5);

        // Apply 2D affine transformation on global/planar UV coordinates
        #ifdef SIMPLE_MATERIAL_HAS_UV_TRANSFORM
            let uv_rot_scale = mat2x2<f32>(
                material_bindings::material.uv_transform_a.xy,
                material_bindings::material.uv_transform_a.zw
            );
            let uv_translation = material_bindings::material.uv_transform_b.xy;

            texture_uv = (uv_rot_scale * texture_uv) + uv_translation;
        #endif

        // Example of texture sampling (adapt with your actual binding names)
        let texture_color = textureSample(material_bindings::base_color_texture, material_bindings::base_color_sampler, texture_uv);

        base_color *= texture_color;
    }

    #ifdef VERTEX_COLORS
        out.color = vec4<f32>(functions::srgb_to_rgb_simple(in.color.xyz * base_color.xyz), 1.0);
    #else
        out.color = vec4<f32>(functions::srgb_to_rgb_simple(base_color.xyz), 1.0);
    #endif

    return out;
}
