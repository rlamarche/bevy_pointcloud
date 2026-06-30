#define_import_path bevy_pointcloud::forward_io

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


struct FragmentOutput {
    @location(0) color: vec4<f32>,
}
