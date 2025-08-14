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

@group(0) @binding(0) var depth_texture: texture_depth_2d;
@group(0) @binding(1) var attribute_texture: texture_2d<f32>;



struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}


@fragment
fn fragment(in: FullscreenVertexOutput) -> FragmentOutput {
    let stored_depth = textureLoad(depth_texture, vec2<i32>(in.position.xy), 0);
    let stored_color = textureLoad(attribute_texture, vec2<i32>(in.position.xy), 0);

    var output: FragmentOutput;


    if (stored_depth == 0) {
        discard;
    }

    output.color = stored_color / stored_color.a;
    output.color.a = 1.0;

    output.depth = stored_depth;

    return output;
}
