//! A shader showing how to use the vertex position data to output the
//! stencil in the right position

// First we import everything we need from bevy_pbr
// A 2d shader would be vevry similar but import from bevy_sprite instead
#import bevy_pbr::mesh_view_bindings as view_bindings
#import bevy_pbr::mesh_functions::mesh_position_local_to_world

#import bevy_pbr::view_transformations::position_world_to_clip
#import bevy_pbr::view_transformations::position_world_to_view
#import bevy_pbr::view_transformations::position_view_to_clip
#import bevy_pbr::view_transformations::position_view_to_ndc

struct Vertex {
    // This is needed if you are using batching and/or gpu preprocessing
    // It's a built in so you don't need to define it in the vertex layout
    @builtin(instance_index) instance_index: u32,
    // Like we defined for the vertex layout
    // position is at location 0
    @location(0) position: vec3<f32>,

    @location(3) i_pos_size: vec4<f32>,
    @location(4) i_color: vec4<f32>,
};

// This is the output of the vertex shader and we also use it as the input for the fragment shader
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) view_position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) log_depth: f32,
    @location(4) v_radius: f32,
};

@group(2) @binding(0)
var<uniform> world_from_local: mat4x4<f32>;

const PI: f32 = 3.14159265358979323846264338327950288;

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    let center = vertex.i_pos_size.xyz;
    let point_size = vertex.i_pos_size.w;

    let viewport = view_bindings::view.viewport;


    let size = vec2(2.0 * point_size / viewport[2], 2.0 * point_size / viewport[3]);

    let world_position = mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.i_pos_size.xyz, 1.0));
    let clip_position = position_world_to_clip(world_position.xyz);
    var view_position = position_world_to_view(world_position.xyz);

    // TODO pass camera fov
    let fov = PI / 4.0;
    let slope = tan(fov / 2.0);
    let proj_factor = -0.5 * viewport[3] / (slope * view_position.z);
    var v_radius = point_size / proj_factor;

    var out: VertexOutput;

    let offset = vertex.position.xy * size;
    out.clip_position = clip_position + vec4<f32>(offset, 0, 0);
    out.view_position = view_position;
    out.color = vertex.i_color;
    out.uv = vertex.position.xy + vec2(0.5);
    out.log_depth = log2(-view_position.z);
    out.v_radius = v_radius;

	#ifdef HQ_DEPTH_PASS
		let original_depth = clip_position.w;
		let adjusted_depth = original_depth + 2.0 * v_radius;
		let adjust = adjusted_depth / original_depth;
        view_position *= adjust;

        out.clip_position = position_view_to_clip(view_position) + vec4<f32>(offset, 0, 0);
	#endif

    return out;
}


@group(3) @binding(0)
var depth_texture: texture_depth_2d;

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    let u = 2.0 * in.uv.x - 1.0;
    let v = 2.0 * in.uv.y - 1.0;
    let cc = u*u + v*v;
    if(cc > 1.0){
        discard;
    }


    var output: FragmentOutput;

    output.color = vec4(in.color.xyz, 1.0);
    output.depth = in.clip_position.z;

#ifdef PARABOLOID_POINT_SHAPE
    let v_radius = in.v_radius;
    let wi = 0.0 - cc;
    var pos = in.view_position;

    pos.z += wi * v_radius;
    let linear_depth = -pos.z;
    let clip_pos = position_view_to_ndc(pos);
    let exp_depth = clip_pos.z * 2.0 - 1.0;

    output.depth = clip_pos.z;
#endif

#ifdef WEIGHTED_SPLATS
    let distance = sqrt(cc);
    var weight = max(0.0, 1.0 - distance);
    weight = pow(weight, 1.5);

    output.color = vec4(in.color.xyz * weight, weight);
#endif

    return output;
}
