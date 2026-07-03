#import bevy_pbr::{
    mesh_view_bindings::view,
    view_transformations::{
        position_world_to_clip,
        position_world_to_view,
        position_view_to_clip,
    },
}

#import bevy_pointcloud::{
    forward_io::{ShapeInput, InstanceInput, VertexOutput, FragmentOutput},
    functions,
    pointcloud_bindings::pointcloud,
    pointcloud_functions,
    simple_material_types as material_types,
    simple_material_bindings as material_bindings,
}


@vertex
fn vertex(
    shape: ShapeInput,
    vertex: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    out.instance_position = vertex.position;
    #ifdef VERTEX_NORMALS
        out.world_normal = pointcloud_functions::mesh_normal_local_to_world(vertex.normal);
    #endif

    let world_from_local = pointcloud_functions::get_world_from_local();


    #ifdef VERTEX_POSITIONS
        var world_position: vec3<f32>;

        #ifdef SIMPLE_MATERIAL_SHAPE_ORIENTATION_FACE_NORMAL
            #ifdef VERTEX_TANGENTS
                world_position = functions::compute_world_position_oriented_with_tangent(
                    vertex.position,
                    vertex.normal,
                    vertex.tangent.xyz,
                    world_from_local,
                    shape.position,
                    material_bindings::material.point_size
                );
            #else
                world_position = functions::compute_world_position_oriented(
                    vertex.position,
                    vertex.normal,
                    world_from_local,
                    shape.position,
                    material_bindings::material.point_size
                );
            #endif
        #else
            world_position = functions::compute_world_position(
                vertex.position,
                world_from_local,
                shape.position,
                material_bindings::material.point_size
            );
        #endif

        out.world_position = vec4<f32>(world_position, 1.0);
        out.position = view.clip_from_world * out.world_position;
    #endif

    #ifdef SHAPE_UVS_A
        out.shape_uv = shape.uv;
    #endif

    #ifdef VERTEX_COLORS
        out.color = vertex.color;
    #endif

    var base_uv = vec2<f32>(0.5, 0.5); // Default fallback center

    #ifdef VERTEX_UVS
        base_uv = vertex.uv;
    #endif

    #ifdef SIMPLE_MATERIAL_UV_MAPPING_COMBINED
        // we need to interpolate our UVs using the point size
        // This transformation is affine, it's ok to do it here.
        var center_uv = base_uv;

        let uv_size = material_bindings::material.point_size;
        let half_uv_size = uv_size * 0.5;

        let min_uv = center_uv - half_uv_size;
        let max_uv = center_uv + half_uv_size;

        base_uv = mix(min_uv, max_uv, shape.uv);
    #endif

    #ifdef SIMPLE_MATERIAL_UV_MAPPING_PLANAR
        var aabb_size = pointcloud.aabb_max.xyz - pointcloud.aabb_min.xyz;

        // prevent zeros and NaN after division
        aabb_size = max(aabb_size, vec3<f32>(0.00001));

        let center_normalized = (vertex.position - pointcloud.aabb_min.xyz) / aabb_size;

        let half_size_uv = (material_bindings::material.point_size / 2.0) / aabb_size;

        let axis_u = material_bindings::material.uv_u;
        let axis_v = material_bindings::material.uv_v;

        let min_uv = vec2<f32>(
            dot(axis_u, center_normalized - half_size_uv),
            dot(axis_v, center_normalized - half_size_uv),
        );
        let max_uv = vec2<f32>(
            dot(axis_u, center_normalized + half_size_uv),
            dot(axis_v, center_normalized + half_size_uv),
        );

        let shape_uv_world_space = vec2<f32>(shape.uv.x, 1.0 - shape.uv.y);

        base_uv = mix(min_uv, max_uv, shape_uv_world_space);
    #endif

    // Apply 2D transformation matrix on the global/planar coordinates.
    // This transformation is affine, it's ok to do it here.
    #ifdef SIMPLE_MATERIAL_HAS_UV_TRANSFORM
        out.uv = (material_bindings::material.uv_transform * vec3<f32>(base_uv, 1.0)).xy;
    #else
        out.uv = base_uv;
    #endif

    return out;
}

@fragment
fn fragment(
    vertex_output: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var in = vertex_output;
    var out: FragmentOutput;

    #ifdef SHAPE_UVS_A
        #ifdef SIMPLE_MATERIAL_SHAPE_RADIUS
        // Perfect circle
        let dist = distance(in.shape_uv, vec2<f32>(0.5, 0.5));

        if dist > material_bindings::material.shape_radius {
            #ifdef DEBUG_UV
                    return vec4<f32>(in.uv, 0.0, 1.0);
            #else // DEBUG
                    discard;
            #endif // DEBUG
        }
        #endif // SIMPLE_MATERIAL_SHAPE_RADIUS
    #endif // SHAPE_UVS_A

    var base_color = material_bindings::material.base_color;

    #ifdef SIMPLE_MATERIAL_GRADIENT

        let projected_dist = dot(in.world_position.xyz, material_bindings::material.gradient_direction);
        let range = material_bindings::material.gradient_end - material_bindings::material.gradient_start;
        var t = 0.0;
        if range > 0.0 {
            t = clamp((projected_dist - material_bindings::material.gradient_start) / range, 0.0, 1.0);
        }

        base_color = functions::evaluate_gradient(
            material_bindings::material.base_color,
            material_bindings::material.end_color,
            material_bindings::material.color_stops,
        t);
    #endif // SIMPLE_MATERIAL_GRADIENT

    if ((material_bindings::material.flags & material_types::SIMPLE_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT) != 0u) {


        // We determine which UV coordinates to use for the texture sampling
        var texture_uv = in.uv; //vec2<f32>(0.5, 0.5);

        #ifdef SIMPLE_MATERIAL_UV_MAPPING_POINT_CLOUD
            // The whole point/quad gets a single flat color sampled from the global texture
            texture_uv = in.uv;
        #endif

        #ifdef SIMPLE_MATERIAL_UV_MAPPING_POINT_SHAPE
            // The entire texture is mapped and repeated onto every single individual point/quad
            texture_uv = in.shape_uv;
        #endif

        #ifdef SIMPLE_MATERIAL_UV_MAPPING_PLANAR
            // Planar is a global mapping style, so it behaves like PointCloud in the fragment
            texture_uv = in.uv;
        #endif

        // Apply 2D transformation matrix on the global/planar coordinates
        #ifdef SIMPLE_MATERIAL_HAS_UV_TRANSFORM
            texture_uv = (material_bindings::material.uv_transform * vec3<f32>(texture_uv, 1.0)).xy;
        #endif

        // Example of texture sampling (adapt with your actual binding names)
        let texture_color = textureSample(material_bindings::base_color_texture, material_bindings::base_color_sampler, texture_uv);

        base_color *= texture_color;
    }

    #ifdef VERTEX_COLORS
        out.color = vec4<f32>(functions::srgb_to_rgb_simple(in.color.xyz * base_color.xyz), 1.0);
    #else
        out.color = vec4<f32>(functions::srgb_to_rgb_simple(base_color.xyz), 1.0);
    #endif

    return out;
}
