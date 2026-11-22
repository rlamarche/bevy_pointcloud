struct ShapeInput {
    @builtin(instance_index) instance_index: u32,
    @if(SHAPE_POSITIONS)
    @location(0) position: vec3<f32>,
    @if(SHAPE_NORMALS)
    @location(1) normal: vec3<f32>,
    @if(SHAPE_UVS_A)
    @location(2) uv: vec2<f32>,
}

struct InstanceInput {
    @if(VERTEX_POSITIONS)
    @location(3) position: vec3<f32>,
    @if(INSTANCE_UVS_A)
    @location(4) uv: vec2<f32>,
    @if(INSTANCE_UVS_B)
    @location(5) uv_b: vec2<f32>,
    @if(INSTANCE_NORMALS)
    @location(6) normal: vec3<f32>,
    @if(VERTEX_TANGENTS)
    @location(7) tangent: vec4<f32>,
    @if(VERTEX_COLORS)
    @location(8) color: vec4<f32>,
}

struct VertexOutput {
    // This is `clip position` when the struct is used as a vertex stage output
    // and `frag coord` when used as a fragment stage input
    @builtin(position) position: vec4<f32>,

    @if(VERTEX_UVS_A)
    @location(0) uv: vec2<f32>,

    @if(VERTEX_UVS_B)
    @location(1) uv_b: vec2<f32>,

    @if(NORMAL_PREPASS_OR_DEFERRED_PREPASS)
    @location(2) world_normal: vec3<f32>,
    @if(NORMAL_PREPASS_OR_DEFERRED_PREPASS && VERTEX_TANGENTS)
    @location(3) world_tangent: vec4<f32>,

    @location(4) world_position: vec4<f32>,
    @if(MOTION_VECTOR_PREPASS)
    @location(5) previous_world_position: vec4<f32>,

    @if(UNCLIPPED_DEPTH_ORTHO_EMULATION)
    @location(6) unclipped_depth: f32,
    @if(VERTEX_OUTPUT_INSTANCE_INDEX)
    @interpolate(flat)
    @location(7) instance_index: u32,

    @if(VERTEX_COLORS)
    @location(8) color: vec4<f32>,

    @if(VISIBILITY_RANGE_DITHER)
    @interpolate(flat)
    @location(9) visibility_range_dither: i32,

    @if(SHAPE_UVS_A)
    @location(10) shape_uv: vec2<f32>,
}

// @if(PREPASS_FRAGMENT)
// struct FragmentOutput {
//     @if(NORMAL_PREPASS)
//     @location(0) normal: vec4<f32>,
//
//     @if(MOTION_VECTOR_PREPASS)
//     @location(1) motion_vector: vec2<f32>,
//
//     @if(DEFERRED_PREPASS)
//     @location(2) deferred: vec4<u32>,
//     @if(DEFERRED_PREPASS)
//     @location(3) deferred_lighting_pass_id: u32,
//
//     @if(UNCLIPPED_DEPTH_ORTHO_EMULATION)
//     @builtin(frag_depth) frag_depth: f32,
// }
