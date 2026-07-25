#define_import_path bevy_pointcloud::prepass_io

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
#ifdef INSTANCE_UVS_A
    @location(4) uv: vec2<f32>,
#endif
#ifdef INSTANCE_UVS_B
    @location(5) uv_b: vec2<f32>,
#endif
#ifdef VERTEX_NORMALS
    @location(6) normal: vec3<f32>,
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

#ifdef VERTEX_UVS_A
    @location(0) uv: vec2<f32>,
#endif

#ifdef VERTEX_UVS_B
    @location(1) uv_b: vec2<f32>,
#endif

#ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
    @location(2) world_normal: vec3<f32>,
#ifdef VERTEX_TANGENTS
    @location(3) world_tangent: vec4<f32>,
#endif
#endif // NORMAL_PREPASS_OR_DEFERRED_PREPASS

    @location(4) world_position: vec4<f32>,
#ifdef MOTION_VECTOR_PREPASS
    @location(5) previous_world_position: vec4<f32>,
#endif

#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    @location(6) unclipped_depth: f32,
#endif // UNCLIPPED_DEPTH_ORTHO_EMULATION
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    @location(7) instance_index: u32,
#endif

#ifdef VERTEX_COLORS
    @location(8) color: vec4<f32>,
#endif

#ifdef VISIBILITY_RANGE_DITHER
    @location(9) @interpolate(flat) visibility_range_dither: i32,
#endif  // VISIBILITY_RANGE_DITHER

#ifdef SHAPE_UVS_A
    @location(10) shape_uv: vec2<f32>,
#endif
}

// #ifdef PREPASS_FRAGMENT
// struct FragmentOutput {
// #ifdef NORMAL_PREPASS
//     @location(0) normal: vec4<f32>,
// #endif

// #ifdef MOTION_VECTOR_PREPASS
//     @location(1) motion_vector: vec2<f32>,
// #endif

// #ifdef DEFERRED_PREPASS
//     @location(2) deferred: vec4<u32>,
//     @location(3) deferred_lighting_pass_id: u32,
// #endif

// #ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
//     @builtin(frag_depth) frag_depth: f32,
// #endif // UNCLIPPED_DEPTH_ORTHO_EMULATION
// }
// #endif //PREPASS_FRAGMENT
