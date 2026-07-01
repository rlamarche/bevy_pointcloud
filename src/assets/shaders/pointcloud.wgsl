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
