// #import bevy_pbr::mesh_view_bindings::view
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
}

// #ifdef PREPASS_PIPELINE
// #import bevy_pbr::{
//     prepass_io::{VertexOutput, FragmentOutput},
//     pbr_deferred_functions::deferred_output,
// }
// #else
// #import bevy_pbr::{
//     forward_io::{VertexOutput, FragmentOutput},
//     pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
//     pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
// }
// #endif


@vertex
fn vertex(
    shape: ShapeInput,
    vertex: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    // the instance is always the first
    let world_from_local = pointcloud_functions::get_world_from_local();

#ifdef VERTEX_NORMALS
    out.world_normal = pointcloud_functions::mesh_normal_local_to_world(vertex.normal);
#endif

#ifdef VERTEX_POSITIONS
    let world_position = pointcloud_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position.xyz, 1.0));
    var view_position = position_world_to_view(world_position.xyz);

    let radius = 0.025;
    let offset = shape.position.xy * radius;
    out.position = position_view_to_clip(view_position + vec3<f32>(offset, 0.0));

    // out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    // out.position = position_world_to_clip(out.world_position.xyz);
#endif

#ifdef SHAPE_UVS_A
    out.shape_uv = shape.uv;
#endif

#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif

    // let right = vec3<f32>(view.world_from_view[0][0], view.world_from_view[0][1], view.world_from_view[0][2]);
    // let up    = vec3<f32>(view.world_from_view[1][0], view.world_from_view[1][1], view.world_from_view[1][2]);

    // // let right = normalize(vec3<f32>(view.clip_from_world[0][0], view.clip_from_world[1][0], view.clip_from_world[2][0]));
    // // let up = normalize(vec3<f32>(view.clip_from_world[0][1], view.clip_from_world[1][1], view.clip_from_world[2][1]));

    // let point_size = 0.025;

    // // Compute world position
    // let world_position = vertex.position
    //                    + right * (shape.position.x * point_size)
    //                    + up    * (shape.position.y * point_size);

    // // Compute clip position from world position
    // out.position = view.clip_from_world * vec4<f32>(world_position, 1.0);
    // // TODO: sending shape uv for the moment, but might be a problem for texturing
    // out.shape_uv = shape.uv;

    // #ifdef VERTEX_COLORS
    //     out.color = vertex.color;
    // #endif

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
        // Perfect circle (TODO make this parametrized in the material)
        let dist = distance(in.shape_uv, vec2<f32>(0.5, 0.5));

        if dist > 0.5 {
            #ifdef DEBUG_UV
                    return vec4<f32>(in.uv, 0.0, 1.0);
            #else // DEBUG
                    discard;
            #endif // DEBUG
        }
    #endif SHAPE_UVS_A

    #ifdef VERTEX_COLORS
        out.color = vec4<f32>(functions::srgb_to_rgb_simple(in.color.xyz), 1.0);
    #else
        out.color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    #endif

    return out;
}
