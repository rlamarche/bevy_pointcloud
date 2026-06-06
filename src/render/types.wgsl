#define_import_path bevy_pointcloud::types

struct Vertex {
    // This is needed if you are using batching and/or gpu preprocessing
    // It's a built in so you don't need to define it in the vertex layout
    @builtin(instance_index) instance_index: u32,
    // Like we defined for the vertex layout
    // position is at location 0
    @location(0) position: vec3<f32>,

    @location(1) i_pos_size: vec4<f32>,
    @location(2) i_color: vec4<f32>,
};

// This is the output of the vertex shader and we also use it as the input for the fragment shader
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) view_position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) log_depth: f32,
    @location(4) radius: f32,
};

#ifdef IS_OCTREE

struct OctreeNode {
    spacing: f32,
    level: u32,
    center: vec3<f32>,
    half_extents: vec3<f32>,
};

struct OctreeEntity {
    octree_index: u32,
#ifdef SIXTEEN_BYTE_ALIGNMENT
    // WebGL2 structs must be 16 byte aligned.
    _webgl2_padding: vec3<f32>,
#endif
};

#endif
