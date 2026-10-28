// Portions of this shader are adapted from Potree (https://github.com/potree/potree)
// Copyright (c) 2011-2020, Markus Schütz
// Licensed under BSD 2-Clause (see THIRD_PARTY_LICENSES.md)


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

#import bevy_pointcloud::{
    simple_material_types as material_types,
}

struct FragmentOutput {
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
    nb_edl_neighbours: u32,
    edl_radius: f32,
    edl_strength: f32,
    clipping_planes: array<vec4<f32>, 8>,
    edl_neighbours: array<vec4<f32>, 8>,
}

@group(#{POINTCLOUD_BIND_GROUP}) @binding(3) var<uniform> view_settings: material_types::PointCloudViewSettings;


#ifdef RENDER_MODE_EDL

fn response(depth: f32, uv: vec2<f32>) -> f32{
	let uv_radius = view_settings.edl_radius / vec2<f32>(view.viewport[2], view.viewport[3]);

	var sum: f32 = 0.0;

	for(var i: u32 = 0u; i < view_settings.nb_edl_neighbours; i++) {
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

    #ifdef POINTCLOUD_PASS_EDL_NORMALIZE
        output.color = stored_color / stored_color.a;

        #ifdef RENDER_MODE_EDL
            let edl_depth = textureLoad(depth_texture, vec2<i32>(in.position.xy), 0).g;

            let res = response(edl_depth, in.uv);
            let shade = exp(-res * 300.0 * view_settings.edl_strength);

            output.color = output.color * shade;
        #endif // RENDER_MODE_EDL
    #endif

    #ifdef POINTCLOUD_PASS_XRAY_NORMALIZE
        output.color = stored_color / stored_color.a;
        // TODO: make it configurable
        output.color.a = 0.9; //stored_color.a;
    #endif

    return output;
}
