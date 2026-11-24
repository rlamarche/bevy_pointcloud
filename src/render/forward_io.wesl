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
    @if(INSTANCE_NORMALS)
    @location(4) normal: vec3<f32>,
    @if(INSTANCE_UVS_A)
    @location(5) uv: vec2<f32>,
    @if(INSTANCE_UVS_B)
    @location(6) uv_b: vec2<f32>,
    @if(VERTEX_TANGENTS)
    @location(7) tangent: vec4<f32>,
    @if(VERTEX_COLORS)
    @location(8) color: vec4<f32>,
}

struct VertexOutput {
    // This is `clip position` when the struct is used as a vertex stage output
    // and `frag coord` when used as a fragment stage input
    @builtin(position) position: vec4<f32>,
    @if(POINTCLOUD_NEEDS_WORLD_POSITION)
    @location(0) world_position: vec4<f32>,
    @if(POINTCLOUD_NEEDS_NORMAL)
    @location(1) world_normal: vec3<f32>,
    @if(POINTCLOUD_NEEDS_POINT_VIEW_POSITION)
    @location(2) point_view_position: vec3<f32>,
    @location(3) view_position: vec3<f32>,
    @location(4) radius: f32,
    @if(POINTCLOUD_NEEDS_LOG_DEPTH)
    @interpolate(flat)
    @location(5) log_depth: f32,
    @if(POINTCLOUD_NEEDS_UVS && VERTEX_UVS_A)
    @location(6) uv: vec2<f32>,
    @if(POINTCLOUD_NEEDS_UVS && VERTEX_UVS_B)
    @location(7) uv_b: vec2<f32>,
    @if(POINTCLOUD_NEEDS_TANGENT && VERTEX_TANGENTS)
    @location(8) world_tangent: vec4<f32>,
    @if(VERTEX_COLORS)
    @location(9) color: vec4<f32>,
    @if(VERTEX_OUTPUT_INSTANCE_INDEX)
    @interpolate(flat)
    @location(10) instance_index: u32,
    @if(VISIBILITY_RANGE_DITHER)
    @interpolate(flat)
    @location(11) visibility_range_dither: i32,
    @location(12) shape_uv: vec2<f32>,
}

// struct FragmentOutput {
//     @location(0) color: vec4<f32>,
// }
