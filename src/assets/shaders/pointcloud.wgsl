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

struct ShapeInput {
    @builtin(instance_index) instance_index: u32,
#ifdef SHAPE_POSITIONS
    @location(0) position: vec3<f32>,
#endif
#ifdef SHAPE_NORMALS
    @location(1) normal: vec3<f32>,
#endif
#ifdef SHAPE_UVS_A
    @location(2) uv: vec2<f32>,
#endif
}

struct InstanceInput {
#ifdef VERTEX_POSITIONS
    @location(3) position: vec3<f32>,
#endif
#ifdef VERTEX_NORMALS
    @location(4) normal: vec3<f32>,
#endif
#ifdef VERTEX_UVS
    @location(5) uv: vec2<f32>,
#endif
#ifdef VERTEX_UVS_B
    @location(6) uv_b: vec2<f32>,
#endif
#ifdef VERTEX_TANGENTS
    @location(7) tangent: vec4<f32>,
#endif
#ifdef VERTEX_COLORS
    @location(8) color: vec4<f32>,
#endif
}

// struct VertexOutput {
//     @builtin(position) clip_position: vec4<f32>,
//     @location(0) uv: vec2<f32>,
//     @location(1) color: vec4<f32>,
// }

struct VertexOutput {
    // This is `clip position` when the struct is used as a vertex stage output
    // and `frag coord` when used as a fragment stage input
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
#ifdef VERTEX_UVS_A
    @location(2) uv: vec2<f32>,
#endif
#ifdef VERTEX_UVS_B
    @location(3) uv_b: vec2<f32>,
#endif
#ifdef VERTEX_TANGENTS
    @location(4) world_tangent: vec4<f32>,
#endif
#ifdef VERTEX_COLORS
    @location(5) color: vec4<f32>,
#endif
    @location(6) shape_uv: vec2<f32>,
}

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
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
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
        return in.color;
    #else
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    #endif
}
