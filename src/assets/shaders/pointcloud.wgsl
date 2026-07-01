#import bevy_pbr::{
    mesh_view_bindings::view,
    view_transformations::{
        position_world_to_clip,
        position_world_to_view,
        position_view_to_clip,
    },
}

#import bevy_pointcloud::{
    forward_io::{ShapeInput, InstanceInput, VertexOutput, FragmentOutput},
    functions,
    pointcloud_bindings::pointcloud,
    pointcloud_functions,
    simple_material_types as material_types,
    simple_material_bindings as material_bindings,
}

@vertex
fn vertex(
    shape: ShapeInput,
    vertex: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    out.instance_position = vertex.position;
    #ifdef VERTEX_NORMALS
        out.world_normal = pointcloud_functions::mesh_normal_local_to_world(vertex.normal);
    #endif

    let world_from_local = pointcloud_functions::get_world_from_local();


#ifdef VERTEX_POSITIONS
    let world_position = functions::compute_world_position(
        vertex.position,
        world_from_local,
        shape.position,
        material_bindings::material.point_size
    );

    out.world_position = vec4<f32>(world_position, 1.0);
    out.position = view.clip_from_world * out.world_position;
#endif

#ifdef SHAPE_UVS_A
    out.shape_uv = shape.uv;
#endif

#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif

    return out;
}

@fragment
fn fragment(
    vertex_output: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var in = vertex_output;
    var out: FragmentOutput;

    #ifdef SHAPE_UVS_A
        #ifdef SIMPLE_MATERIAL_SHAPE_RADIUS
        // Perfect circle
        let dist = distance(in.shape_uv, vec2<f32>(0.5, 0.5));

        if dist > material_bindings::material.shape_radius {
            #ifdef DEBUG_UV
                    return vec4<f32>(in.uv, 0.0, 1.0);
            #else // DEBUG
                    discard;
            #endif // DEBUG
        }
        #endif // SIMPLE_MATERIAL_SHAPE_RADIUS
    #endif // SHAPE_UVS_A

    var base_color = material_bindings::material.base_color;

    #ifdef SIMPLE_MATERIAL_GRADIENT

        let projected_dist = dot(in.world_position.xyz, material_bindings::material.gradient_direction);
        let range = material_bindings::material.gradient_end - material_bindings::material.gradient_start;
        var t = 0.0;
        if range > 0.0 {
            t = clamp((projected_dist - material_bindings::material.gradient_start) / range, 0.0, 1.0);
        }

        base_color = evaluate_gradient(material_bindings::material, t);
    #endif // SIMPLE_MATERIAL_GRADIENT

    if ((material_bindings::material.flags & material_types::SIMPLE_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT) != 0u) {
        base_color *=
            textureSample(
                material_bindings::base_color_texture,
                material_bindings::base_color_sampler,
                in.shape_uv,
        );
    }

    #ifdef VERTEX_COLORS
        out.color = vec4<f32>(functions::srgb_to_rgb_simple(in.color.xyz * base_color.xyz), 1.0);
    #else
        out.color = vec4<f32>(functions::srgb_to_rgb_simple(base_color.xyz), 1.0);
    #endif

    return out;
}





// Function to evaluate the multi-stop gradient based on shaderdefs
fn evaluate_gradient(material: material_types::SimplePointCloudMaterial, t: f32) -> vec4<f32> {
    let factor = clamp(t, 0.0, 1.0);

    #ifdef SIMPLE_MATERIAL_GRADIENT
        var current_start_color = material.base_color;
        var current_start_point = 0.0;

        // Cumulative chain for 8 color stops
        #ifdef SIMPLE_MATERIAL_COLOR_STOP_1
            if factor < material.color_stops[0].point {
                let local_t = (factor - current_start_point) / (material.color_stops[0].point - current_start_point);
                return mix(current_start_color, material.color_stops[0].color, local_t);
            }
            current_start_color = material.color_stops[0].color;
            current_start_point = material.color_stops[0].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_2
            if factor < material.color_stops[1].point {
                let local_t = (factor - current_start_point) / (material.color_stops[1].point - current_start_point);
                return mix(current_start_color, material.color_stops[1].color, local_t);
            }
            current_start_color = material.color_stops[1].color;
            current_start_point = material.color_stops[1].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_3
            if factor < material.color_stops[2].point {
                let local_t = (factor - current_start_point) / (material.color_stops[2].point - current_start_point);
                return mix(current_start_color, material.color_stops[2].color, local_t);
            }
            current_start_color = material.color_stops[2].color;
            current_start_point = material.color_stops[2].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_4
            if factor < material.color_stops[3].point {
                let local_t = (factor - current_start_point) / (material.color_stops[3].point - current_start_point);
                return mix(current_start_color, material.color_stops[3].color, local_t);
            }
            current_start_color = material.color_stops[3].color;
            current_start_point = material.color_stops[3].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_5
            if factor < material.color_stops[4].point {
                let local_t = (factor - current_start_point) / (material.color_stops[4].point - current_start_point);
                return mix(current_start_color, material.color_stops[4].color, local_t);
            }
            current_start_color = material.color_stops[4].color;
            current_start_point = material.color_stops[4].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_6
            if factor < material.color_stops[5].point {
                let local_t = (factor - current_start_point) / (material.color_stops[5].point - current_start_point);
                return mix(current_start_color, material.color_stops[5].color, local_t);
            }
            current_start_color = material.color_stops[5].color;
            current_start_point = material.color_stops[5].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_7
            if factor < material.color_stops[6].point {
                let local_t = (factor - current_start_point) / (material.color_stops[6].point - current_start_point);
                return mix(current_start_color, material.color_stops[6].color, local_t);
            }
            current_start_color = material.color_stops[6].color;
            current_start_point = material.color_stops[6].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_8
            if factor < material.color_stops[7].point {
                let local_t = (factor - current_start_point) / (material.color_stops[7].point - current_start_point);
                return mix(current_start_color, material.color_stops[7].color, local_t);
            }
            current_start_color = material.color_stops[7].color;
            current_start_point = material.color_stops[7].point;
        #endif

        // Final interpolation between the last active stop and end_color
        let final_t = (factor - current_start_point) / (1.0 - current_start_point);
        return mix(current_start_color, material.end_color, final_t);

    #else
        return material.base_color;
    #endif
}
