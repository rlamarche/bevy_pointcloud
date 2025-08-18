// Portions of this shader are adapted from Potree (https://github.com/potree/potree)
// Copyright (c) 2011-2020, Markus Schütz
// Licensed under BSD 2-Clause (see THIRD_PARTY_LICENSES.md)


// This shader computes the chromatic aberration effect

// Since post processing is a fullscreen effect, we use the fullscreen vertex shader provided by bevy.
// This will import a vertex shader that renders a single fullscreen triangle.
//
// A fullscreen triangle is a single triangle that covers the entire screen.
// The box in the top left in that diagram is the screen. The 4 x are the corner of the screen
//
// Y axis
//  1 |  x-----x......
//  0 |  |  s  |  . ´
// -1 |  x_____x´
// -2 |  :  .´
// -3 |  :´
//    +---------------  X axis
//      -1  0  1  2  3
//
// As you can see, the triangle ends up bigger than the screen.
//
// You don't need to worry about this too much since bevy will compute the correct UVs for you.
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

#ifdef MULTISAMPLED

@group(0) @binding(0) var depth_texture: texture_multisampled_2d<f32>;
@group(0) @binding(1) var attribute_texture: texture_multisampled_2d<f32>;

#else // MULTISAMPLED

@group(0) @binding(0) var depth_texture: texture_2d<f32>;
@group(0) @binding(1) var attribute_texture: texture_2d<f32>;

#endif // MULTISAMPLED

struct FragmentOutput {
    @location(0) color: vec4<f32>,
}


#ifdef USE_EDL

struct EyeDomeLightingUniform {
    strength: f32,
    radius: f32,
    screen_width: f32,
    screen_height: f32,
};

@group(1) @binding(0)
var<uniform> edl: EyeDomeLightingUniform;

@group(1) @binding(1)
var<storage, read> edl_neighbours: array<vec2<f32>, #{NEIGHBOUR_COUNT}u>;

fn response(depth: f32, uv: vec2<f32>) -> f32{
	let uv_radius = edl.radius / vec2<f32>(edl.screen_width, edl.screen_height);

	var sum: f32 = 0.0;

	for(var i: u32 = 0u; i < #{NEIGHBOUR_COUNT}u; i++){
		var uv_neighbor = uv + uv_radius * edl_neighbours[i];
		uv_neighbor.x *= edl.screen_width;
		uv_neighbor.y *= edl.screen_height;

		let neighbour_depth = textureLoad(depth_texture, vec2<i32>(uv_neighbor), 0).g;

		if(neighbour_depth != 0.0){
			if(depth == 0.0){
				sum += 100.0;
			}else{
				sum += max(0.0, depth - neighbour_depth);
			}
		}
	}

    let a = f32(#{NEIGHBOUR_COUNT}u);
	return sum / a;
}


#endif



@fragment
fn fragment(in: FullscreenVertexOutput) -> FragmentOutput {

#ifdef USE_EDL
    let edl_depth = textureLoad(depth_texture, vec2<i32>(in.position.xy), 0).g;

    let res = response(edl_depth, in.uv);
    let shade = exp(-res * 300.0 * edl.strength);
#endif // USE_EDL

    let depth = textureLoad(depth_texture, vec2<i32>(in.position.xy), 0).r;

    if (depth == 1.0) {
        discard;
    }

    let stored_color = textureLoad(attribute_texture, vec2<i32>(in.position.xy), 0);

    var output: FragmentOutput;
    output.color = stored_color / stored_color.a;

#ifdef USE_EDL
    output.color = output.color * shade;
#endif // USE_EDL

    output.color.a = 1.0;

    return output;
}
