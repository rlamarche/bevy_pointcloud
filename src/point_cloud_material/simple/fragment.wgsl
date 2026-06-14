// Portions of this shader are adapted from Potree (https://github.com/potree/potree)
// Copyright (c) 2011-2020, Markus Schütz
// Licensed under BSD 2-Clause (see THIRD_PARTY_LICENSES.md)

#import bevy_pbr::view_transformations::position_view_to_ndc

#import bevy_pointcloud::functions
#import bevy_pointcloud::types


struct FragmentOutput {
#ifdef DEPTH_PASS
    #ifdef USE_EDL
    @location(0) depth_texture: vec2<f32>,
    #else // USE EDL
    @location(0) depth_texture: f32,
    #endif // USE EDL
#else
    @location(0) color: vec4<f32>,
#endif
    @builtin(frag_depth) depth: f32,
}

@fragment
fn fragment(in: types::VertexOutput) -> FragmentOutput {
    let u = 2.0 * in.uv.x - 1.0;
    let v = 2.0 * in.uv.y - 1.0;
    let cc = u*u + v*v;
    if(cc > 1.0){
        discard;
    }

    // convert the color to linear RGB
    let color = functions::srgb_to_rgb_simple(in.color.xyz);

    var output: FragmentOutput;

    output.depth = in.clip_position.z;

#ifdef DEPTH_PASS
    #ifdef USE_EDL
        output.depth_texture.r = in.clip_position.z;
        output.depth_texture.g = in.log_depth;
    #else // USE_EDL
        output.depth_texture = in.clip_position.z;
    #endif // USE_EDL

    #ifdef PARABOLOID_POINT_SHAPE
    let radius = in.radius;
    let wi = 0.0 - cc;
    var pos = in.view_position;

    pos.z += wi * radius;
    let linear_depth = -pos.z;
    let clip_pos = position_view_to_ndc(pos);
    let exp_depth = clip_pos.z * 2.0 - 1.0;

    output.depth = clip_pos.z;
    #endif
#else // DEPTH_PASS
    output.color = vec4(color, 1.0);

    #ifdef PARABOLOID_POINT_SHAPE
    let radius = in.radius;
    let wi = 0.0 - cc;
    var pos = in.view_position;

    pos.z += wi * radius;
    let linear_depth = -pos.z;
    let clip_pos = position_view_to_ndc(pos);
    let exp_depth = clip_pos.z * 2.0 - 1.0;

    output.depth = clip_pos.z;
    #endif

    #ifdef WEIGHTED_SPLATS
    let distance = sqrt(cc);
    var weight = max(0.0, 1.0 - distance);
    weight = pow(weight, 1.5);

    output.color = vec4(color * weight, weight);
    #endif

#endif // DEPTH_PASS

    return output;
}
