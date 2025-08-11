#import bevy_pbr::mesh_view_bindings as view_bindings

#import bevy_pbr::{
    view_transformations::position_world_to_clip
}

#import bevy_pbr::mesh_functions::{get_world_from_local, mesh_position_local_to_world}

struct Vertex {
    @location(0) position: vec3<f32>,
//    @location(1) normal: vec3<f32>,
//    @location(2) uv: vec2<f32>,

    @location(3) i_pos_size: vec4<f32>,
    @location(4) i_color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@group(2) @binding(0)
var<uniform> world_from_local: mat4x4<f32>;

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    let center = vertex.i_pos_size.xyz;
    let point_size = vertex.i_pos_size.w;

    let view_inverse = view_bindings::view.view_from_world;

    let viewport = view_bindings::view.viewport;
    let size = vec2(2.0 * point_size / viewport[2], 2.0 * point_size / viewport[3]);

    // NOTE: Passing 0 as the instance_index to get_world_from_local() is a hack
    // for this example as the instance_index builtin would map to the wrong
    // index in the Mesh array. This index could be passed in via another
    // uniform instead but it's unnecessary for the example.
    let world_position = mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.i_pos_size.xyz, 1.0));
    let clip_position = position_world_to_clip(world_position.xyz);


    var out: VertexOutput;

    let offset = vertex.position.xy * size;

    out.clip_position = clip_position + vec4<f32>(offset, 0, 0);
    out.color = vertex.i_color;
    out.uv = vertex.position.xy + vec2(0.5);
    return out;
}



@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let dist = distance(in.uv, vec2(0.5));
    if dist > 0.5 {
        discard; // coupe les coins
    }
    let alpha = 1.0 - smoothstep(0.0, 0.5, dist); // alpha = 1 au centre, 0 au bord

    return vec4(in.color.xyz, alpha * in.color.a);
}
