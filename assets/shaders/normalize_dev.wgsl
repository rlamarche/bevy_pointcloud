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
#import bevy_pbr::{
    mesh_view_bindings::view,
}

struct FragmentOutput {
    @builtin(frag_depth) depth: f32,
    @location(0) color: vec4<f32>,
}

#ifdef MULTISAMPLED

@group(#{POINTCLOUD_BIND_GROUP}) @binding(10) var depth_texture: texture_multisampled_2d<f32>;
@group(#{POINTCLOUD_BIND_GROUP}) @binding(11) var attribute_texture: texture_multisampled_2d<f32>;

#else // MULTISAMPLED

@group(#{POINTCLOUD_BIND_GROUP}) @binding(10) var depth_texture: texture_2d<f32>;
@group(#{POINTCLOUD_BIND_GROUP}) @binding(11) var attribute_texture: texture_2d<f32>;

#endif // MULTISAMPLED



struct PointCloudViewSettings {
    nb_clipping_planes: u32,
    _padding: vec3<f32>,
    clipping_planes: array<vec4<f32>, 8>,
    edl_neighbours: array<vec4<f32>, 8>,
}

@group(#{POINTCLOUD_BIND_GROUP}) @binding(3) var<uniform> view_settings: PointCloudViewSettings;


#ifdef RENDER_MODE_EDL

fn response(depth: f32, uv: vec2<f32>) -> f32{
	let uv_radius = /* view_settings.radius*/ 1.4 / vec2<f32>(view.viewport[2], view.viewport[3]);

	var sum: f32 = 0.0;

	for(var i: u32 = 0u; i < 4; i++) {
		var uv_neighbor_0 = uv + uv_radius * view_settings.edl_neighbours[i].xy;
		uv_neighbor_0.x *= view.viewport[2];
		uv_neighbor_0.y *= view.viewport[3];

		let neighbour_depth_0 = textureLoad(depth_texture, vec2<i32>(uv_neighbor_0), 0).g;

		if(neighbour_depth_0 != 0.0){
			if(depth == 0.0){
				sum += 100.0;
			}else{
				sum += max(0.0, depth - neighbour_depth_0);
			}
		}
	}

    let a = f32(4u);
	return sum / a;
}


#endif // RENDER_MODE_EDL

@fragment
fn fragment(in: FullscreenVertexOutput) -> FragmentOutput {
    var output: FragmentOutput;

    let stored_color = textureLoad(attribute_texture, vec2<i32>(in.position.xy), 0);

    if (stored_color.a == 0.0) {
        discard;
    }
    let depth =  textureLoad(depth_texture, vec2<i32>(in.position.xy), 0);

    output.depth = 1.0 - depth.x;
    output.color = stored_color / stored_color.a;

    var edl_strength = 0.4;

    #ifdef RENDER_MODE_EDL
        let edl_depth = textureLoad(depth_texture, vec2<i32>(in.position.xy), 0).g;

        let res = response(edl_depth, in.uv);
        let shade = exp(-res * 300.0 * edl_strength);

        output.color = output.color * shade;
        // output.color = vec4<f32>(shade, shade, shade, 1.0);
    #endif // RENDER_MODE_EDL

    // output.color = vec4<f32>(1.0, 0.0, 0.0, 1.0);

    return output;
}
