#import bevy_pbr::{
    mesh_view_bindings::view,
    view_transformations::position_world_to_view,
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

#ifdef POINTCLOUD_PASS_ATTRIBUTE

#ifdef MULTISAMPLED

@group(#{POINTCLOUD_BIND_GROUP}) @binding(10) var depth_texture: texture_multisampled_2d<f32>;
// @group(2) @binding(1) var attribute_texture: texture_multisampled_2d<f32>;

#else // MULTISAMPLED

@group(#{POINTCLOUD_BIND_GROUP}) @binding(10) var depth_texture: texture_2d<f32>;
// @group(2) @binding(1) var attribute_texture: texture_2d<f32>;

#endif // MULTISAMPLED

#endif // POINTCLOUD_PASS_ATTRIBUTE


#ifdef POINTCLOUD_VIEW_SETTINGS
struct PointCloudViewSettings {
    nb_clipping_planes: u32,
    _padding: vec3<f32>,
    clipping_planes: array<vec4<f32>, 8>,
}

@group(#{POINTCLOUD_BIND_GROUP}) @binding(3) var<uniform> view_settings: PointCloudViewSettings;
#endif // POINTCLOUD_VIEW_SETTINGS


struct FragmentOutput {
#ifdef POINTCLOUD_PASS_DEPTH
    @builtin(frag_depth) depth: f32,
    #ifdef USE_EDL
        @location(0) depth_texture: vec2<f32>,
    #else // USE EDL
        @location(0) depth_texture: f32,
    #endif // USE EDL
#else // POINTCLOUD_PASS_DEPTH
    @location(0) color: vec4<f32>,
#endif // POINTCLOUD_PASS_DEPTH
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

    #ifdef POINTCLOUD_VIEW_SETTINGS
        #ifdef CLIPPING_PLANES
            for (var i: u32 = 0u; i < view_settings.nb_clipping_planes; i ++) {
                let clip_dist = dot(in.world_position.xyz, view_settings.clipping_planes[i].xyz) + view_settings.clipping_planes[i].w;
                if (clip_dist < 0.0) {
                    discard;
                }
            }
        #endif // CLIPPING_PLANES
    #endif // POINTCLOUD_VIEW_SETTINGS

    #ifdef POINTCLOUD_PASS_DEPTH
        #ifdef USE_EDL
            out.depth_texture.r = in.position.z;
            out.depth_texture.g = in.log_depth;
        #else // USE_EDL
            out.depth_texture = in.position.z;
        #endif // USE_EDL

//         #ifdef PARABOLOID_POINT_SHAPE
//             let radius = in.radius;
//             let wi = 0.0 - cc;
//             var pos = in.view_position;
//
//             pos.z += wi * radius;
//             let linear_depth = -pos.z;
//             let clip_pos = position_view_to_ndc(pos);
//             let exp_depth = clip_pos.z * 2.0 - 1.0;
//
//             output.depth = clip_pos.z;
//         #endif // PARABOLOID_POINT_SHAPE
    #else // POINTCLOUD_PASS_DEPTH
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
            #ifdef POINTCLOUD_PASS_ATTRIBUTE
                out.color = vec4<f32>(functions::srgb_to_rgb_simple(in.color.xyz * base_color.xyz), in.position.z);
            #else // POINTCLOUD_PASS_ATTRIBUTE
                out.color = vec4<f32>(functions::srgb_to_rgb_simple(in.color.xyz * base_color.xyz), 1.0);
            #endif // POINTCLOUD_PASS_ATTRIBUTE
        #else
            #ifdef POINTCLOUD_PASS_ATTRIBUTE
                out.color = vec4<f32>(functions::srgb_to_rgb_simple(base_color.xyz), in.position.z);
            #else // POINTCLOUD_PASS_ATTRIBUTE
                out.color = vec4<f32>(functions::srgb_to_rgb_simple(base_color.xyz), 1.0);
            #endif // POINTCLOUD_PASS_ATTRIBUTE
        #endif

//     #ifdef POINTCLOUD_PASS_ATTRIBUTE
//         let uv = 2.0 * in.shape_uv - vec2<f32>(1.0);
//         let cc = dot(uv, uv);
//         let distance = distance(in.shape_uv, vec2<f32>(0.5, 0.5));
//
//         var weight = max(0.0, 1.0 - distance);
//         weight = pow(weight, 1.5);
//
//         out.color = vec4(out.color.rgb * weight, weight * out.color.a);
//     #endif // POINTCLOUD_PASS_ATTRIBUTE

        // #ifdef POINTCLOUD_PASS_ATTRIBUTE
        //     let xy = vec2<i32>(in.position.xy) + vec2<i32>(50);
        //     let depth_color = textureLoad(depth_texture, xy, 0);
        //     out.color = depth_color;
        // #endif

    #endif // POINTCLOUD_PASS_DEPTH

    return out;
}
