#import bevy_pbr::mesh_view_bindings::view

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
}

struct InstanceInput {
    @location(10) instance_position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vertex(
    vertex: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    let right = vec3<f32>(view.world_from_view[0][0], view.world_from_view[0][1], view.world_from_view[0][2]);
    let up    = vec3<f32>(view.world_from_view[1][0], view.world_from_view[1][1], view.world_from_view[1][2]);

    // let right = normalize(vec3<f32>(view.clip_from_world[0][0], view.clip_from_world[1][0], view.clip_from_world[2][0]));
    // let up = normalize(vec3<f32>(view.clip_from_world[0][1], view.clip_from_world[1][1], view.clip_from_world[2][1]));

    let point_size = 0.1;

    // Compute world position
    let world_position = instance.instance_position
                       + right * (vertex.position.x * point_size)
                       + up    * (vertex.position.y * point_size);

    // Compute clip position from world position
    out.clip_position = view.clip_from_world * vec4<f32>(world_position, 1.0);
    out.uv = vertex.uv;

    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Perfect circle
    let dist = distance(in.uv, vec2<f32>(0.5, 0.5));
    if dist > 0.5 {
#ifdef DEBUG_UV
        return vec4<f32>(in.uv, 0.0, 1.0);
#else // DEBUG
        discard;
#endif // DEBUG
    }

    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}
