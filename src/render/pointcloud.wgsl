#import bevy_pbr::mesh_view_bindings as view_bindings

#import bevy_pbr::{
    view_transformations::position_world_to_clip
}


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

//@vertex
//fn vertex(vertex: Vertex) -> VertexOutput {
//    // Centre et scale de l’instance
//    let center = vertex.i_pos_size.xyz;
//    let scale = vertex.i_pos_size.w;
//
//    let view_inverse = view_bindings::view.view_from_world;
//    let right = vec3<f32>(view_inverse[0][0], view_inverse[1][0], view_inverse[2][0]);
//    let up = vec3<f32>(view_inverse[0][1], view_inverse[1][1], view_inverse[2][1]);
//
//    // Offset local sur le quad (vertex.position.xy = [-1..1], ex: -1 à +1)
//    let local_offset = vertex.position.xy * scale;
//
//    // Position dans le monde du vertex (quad orienté face caméra)
//    let world_pos = center + right * local_offset.x + up * local_offset.y;
//
//    var out: VertexOutput;
//    out.clip_position = position_world_to_clip(world_pos);
//    out.color = vertex.i_color;
//    out.uv = vertex.position.xy + vec2(0.5);
//    return out;
//}



@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    let center = vertex.i_pos_size.xyz;
    let point_size = vertex.i_pos_size.w;

    let view_inverse = view_bindings::view.view_from_world;
    let right = vec3<f32>(view_inverse[0][0], view_inverse[1][0], view_inverse[2][0]);
    let up = vec3<f32>(view_inverse[0][1], view_inverse[1][1], view_inverse[2][1]);

    let viewport = view_bindings::view.viewport;
    let size = vec2(2.0 * point_size / viewport[2], 2.0 * point_size / viewport[3]);


    let world_pos = position_world_to_clip(vertex.i_pos_size.xyz);


    var out: VertexOutput;

    let offset = vertex.position.xy * size;

    out.clip_position = world_pos + vec4<f32>(offset, 0, 0);
    out.color = vertex.i_color;
    out.uv = vertex.position.xy /** scale * world_pos.w*/ + vec2(0.5);
    return out;
}



@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
//    return in.color;
    // Optionnel : rendre un disque plutôt qu’un carré
//    let dist = distance(in.uv, vec2(0.5));
//    if dist > 0.5 {
//        discard; // coupe les coins
//    }
//    return vec4(in.color.xyz, log(1.0 + dist));

    let dist = distance(in.uv, vec2(0.5));
    if dist > 0.5 {
        discard; // coupe les coins
    }
    let alpha = 1.0 - smoothstep(0.0, 0.5, dist); // alpha = 1 au centre, 0 au bord

    return vec4(in.color.xyz, alpha * in.color.a);
}
