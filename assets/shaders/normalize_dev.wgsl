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

    // output.color = vec4<f32>(1.0, 0.0, 0.0, 1.0);

    return output;
}
