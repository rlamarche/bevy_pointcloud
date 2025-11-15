// Portions of this shader are adapted from Potree (https://github.com/potree/potree)
// Copyright (c) 2011-2020, Markus Schütz
// Licensed under BSD 2-Clause (see THIRD_PARTY_LICENSES.md)

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

@group(1) @binding(0)
var<uniform> world_from_local: mat4x4<f32>;

struct PointCloudMaterial {
    point_size: f32,
    min_point_size: f32,
    max_point_size: f32,
#ifdef SIXTEEN_BYTE_ALIGNMENT
    // WebGL2 structs must be 16 byte aligned.
    _webgl2_padding: f32
#endif
};

@group(2) @binding(0)
var<uniform> material: PointCloudMaterial;

#ifdef IS_OCTREE

struct OctreeNode {
    spacing: f32,
    level: u32,
    center: vec3<f32>,
    half_extents: vec3<f32>,
    octree_index: u32,
    node_index: u32,
};

struct VisibleNode {
    octree_index: u32,
    node_index: u32,
#ifdef SIXTEEN_BYTE_ALIGNMENT
    // WebGL2 structs must be 16 byte aligned.
    _webgl2_padding_1: vec3<u32>,
    _webgl2_padding_2: vec4<u32>,
#endif
};

@group(3) @binding(0)
var<uniform> octree_node: OctreeNode;

@group(4) @binding(0)
var visible_nodes: texture_2d<u32>;

@group(5) @binding(0)
var<uniform> visible_node: VisibleNode;

#endif

const PI: f32 = 3.14159265358979323846264338327950288;


fn srgb_to_rgb_simple(color: vec3<f32>) -> vec3<f32> {
    return pow(color, vec3<f32>(2.2));
}


#ifdef IS_OCTREE


fn is_bit_set(number: u32, index: u32) -> bool {
    return (number & (1u << index)) != 0u;
}

fn count_one_bits_compat(x: u32) -> u32 {
    var v = x;
    v = v - ((v >> 1u) & 0x55555555u);
    v = (v & 0x33333333u) + ((v >> 2u) & 0x33333333u);
    return (((v + (v >> 4u)) & 0x0F0F0F0Fu) * 0x01010101u) >> 24u;
}

// Fonction auxiliaire : compter combien de bits sont à 1 avant l'index donné
fn count_bits_before(mask: u32, index: u32) -> u32 {
    // Créer un masque pour les bits avant index
    let before_mask = (1u << index) - 1u;

    // TODO add ifdef to use native version if available
    return count_one_bits_compat(mask & before_mask);
//    return countOneBits(mask & before_mask);
}

/**
 * number of 1-bits up to inclusive index position
 * number is treated as if it were an integer in the range 0-255
 *
 */
fn number_of_ones(mask: u32, index: u32) -> u32 {
    var number = mask;
	var num_ones: u32 = 0u;
	var tmp: u32 = 128u;

	for(var i: i32 = 7; i >= 0; i--){

		if(number >= tmp){
			number = number - tmp;

			if(u32(i) <= index){
				num_ones++;
			}
		}

		tmp = tmp / 2u;
	}

	return num_ones;
}

fn get_max_relative_depth(position: vec3<f32>) -> vec2<f32> {

    var current_index = visible_node.node_index;
    var relative_depth: i32 = 0;

    var center = octree_node.center;
    var half_extents = octree_node.half_extents;

    for (var i = 0; i <= 30; i ++) {
        let current_node = textureLoad(visible_nodes, vec2<u32>(current_index, visible_node.octree_index), 0);

        // Décomposer les données
        let children_mask = current_node.r;  // u8 dans le canal R

        // current_node.g est le padding (inutilisé)
        let first_child_index = current_node.b | (current_node.a << 8u);  // u16 reconstruit à partir de B et A

        // Déterminer dans quel octant se trouve la position
        let relative_position = position - center;

        // index3d contient 0 ou 1 pour chaque axe
        let index3d = step(vec3(0.0), relative_position);

        // compute the child_index
        let child_index = u32(round(4.0 * index3d.x + 2.0 * index3d.y + index3d.z));

        // check if a children exists at this index
        if is_bit_set(children_mask, child_index) {
            // compute child offset
            var child_offset: u32 = 0u;
            if child_index > 0 {
                child_offset = count_bits_before(children_mask, child_index);
            }

//            let child_offset = number_of_ones(children_mask, child_index - 1);
            let actual_child_index = first_child_index + child_offset;

            relative_depth ++;

            current_index = actual_child_index;
            half_extents = half_extents  * 0.5;

            let offset = (index3d * 2.0 - 1.0) * half_extents;
            center = center + offset;
        } else {
            let offset = f32(current_node.g) / 10.0 - 10.0;
//            return f32(relative_depth) + offset;
            return vec2(f32(relative_depth), offset);
        }

    }

    return vec2(f32(relative_depth), 0.0);
}

#endif


@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    let center = vertex.i_pos_size.xyz;
    var point_size = material.point_size;

    if (point_size < 0.0) {
        point_size = vertex.i_pos_size.w;
    }

    let viewport = view_bindings::view.viewport;



    // Compute world & view position of the point instance (applying the world_from_local matrix)
    let world_position = mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.i_pos_size.xyz, 1.0));
    var view_position = position_world_to_view(world_position.xyz);

#ifdef IS_OCTREE
    // Get the fov from projection matrix
    let f = view_bindings::view.clip_from_view[1][1];
    let fov = 2.0 * atan(1.0 / f);
    let slope = tan(fov / 2.0);
    var proj_factor = -0.5 * viewport[3] / (slope * view_position.z);

    // TODO precalculate it on cpu
    let model_view = view_bindings::view.view_from_world * world_from_local;

	let scale = length(
		model_view * vec4(0, 0, 0, 1) -
		model_view * vec4(octree_node.spacing, 0, 0, 1)
	) / octree_node.spacing;
	proj_factor = proj_factor * scale;


    let max_relative_depth = get_max_relative_depth(vertex.i_pos_size.xyz);
    let attenuation = pow(2.0, max_relative_depth.x + max_relative_depth.y);
//    let attenuation = 0.5 * pow(1.3, max_relative_depth.x + max_relative_depth.y);

    var radius = octree_node.spacing * 1.7 / attenuation;
    radius = radius * proj_factor;

	radius = max(material.min_point_size, radius);
	radius = min(material.max_point_size, radius);

	radius = radius / proj_factor;
#else
    // Compute radius to size the point correctly with viewport size
    let radius = point_size / min(viewport[2], viewport[3]);
#endif

    // Compute the offset to apply for creating a quad
    let offset = vertex.position.xy * radius;

    // Apply the offset to the view position and compute clip position
    let clip_position = position_view_to_clip(view_position + vec3<f32>(offset, 0.0));

    var out: VertexOutput;

    out.clip_position = clip_position;
    out.view_position = view_position;

    out.color = vertex.i_color;

#ifdef IS_OCTREE
#ifdef DEBUG_COLOR
    var debug_color = vec3<f32>(1.0, 1.0, 1.0);

//    let absolute_depth = max_relative_depth + octree_node.level;
//
//    if absolute_depth == 0u {
//        debug_color = vec3<f32>(1.0, 0.0, 0.0); // Rouge = problème !
//    } else if absolute_depth == 1u {
//        debug_color = vec3<f32>(1.0, 1.0, 0.0); // Jaune
//    } else if absolute_depth == 2u {
//        debug_color = vec3<f32>(0.0, 1.0, 0.0); // Vert
//    } else {
//        debug_color = vec3<f32>(0.0, 0.0, f32(absolute_depth) / 10.0); // Bleu = profond
//    }
//
//    out.color = vec4<f32>(debug_color, 1.0);

    let absolute_depth = max_relative_depth.y;

    if absolute_depth < 1.0 {
        debug_color = vec3<f32>(absolute_depth + 1.0, 0.0, 0.0); // Rouge = problème !
    } else if absolute_depth < 2.0 {
        debug_color = vec3<f32>(absolute_depth, absolute_depth, 0.0); // Jaune
    } else if absolute_depth < 3.0 {
        debug_color = vec3<f32>(0.0, absolute_depth, 0.0); // Vert
    } else {
        debug_color = vec3<f32>(0.0, 0.0, f32(absolute_depth) / 10.0); // Bleu = profond
    }

    out.color = vec4<f32>(debug_color, 1.0);
#endif // DEBUG_COLOR
#endif // IS_OCTREE

    out.uv = vertex.position.xy + vec2(0.5);
    out.log_depth = log2(-view_position.z);
    out.radius = radius;

	#ifdef HQ_DEPTH_PASS
		let original_depth = clip_position.w;
		let adjusted_depth = original_depth + 2.0 * radius;
		let adjust = adjusted_depth / original_depth;

        view_position *= adjust;
        view_position += vec3<f32>(offset, 0.0);

        out.clip_position = position_view_to_clip(view_position);
	#endif

    return out;
}

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
fn fragment(in: VertexOutput) -> FragmentOutput {
    let u = 2.0 * in.uv.x - 1.0;
    let v = 2.0 * in.uv.y - 1.0;
    let cc = u*u + v*v;
    if(cc > 1.0){
        discard;
    }

    // convert the color to linear RGB
    let color = srgb_to_rgb_simple(in.color.xyz);

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
