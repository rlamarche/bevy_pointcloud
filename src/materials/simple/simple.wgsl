#import bevy_render::view::position_view_to_world;

#import bevy_pbr::{
    mesh_view_bindings::view,
    mesh_functions,
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

    // We assume VERTEX_POSITIONS & SHAPE_POSITIONS are set.

    out.instance_index = 0;

    out.instance_position = vertex.position;

    #ifdef VERTEX_NORMALS
        out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, out.instance_index);
    #endif

    let world_from_local = pointcloud_functions::get_world_from_local();

    // compute the world position of point coordinates
    let world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    var view_vertex_position: vec3<f32>;

    let view_position = position_world_to_view(world_position.xyz);

    var radius: f32;
    let radius_scale = functions::extract_max_scale(world_from_local);

    #ifdef SIMPLE_MATERIAL_POINT_SIZE_SCREEN
        radius = functions::compute_screen_space_point_size(
            view_position,
            material_bindings::material.point_size,
            material_bindings::material.min_point_size,
            material_bindings::material.max_point_size
        );
    #endif

    #ifdef SIMPLE_MATERIAL_UV_POINT_SIZE_SCREEN_LOCAL
        radius = functions::compute_screen_space_point_size(
            view_position,
            material_bindings::material.point_size * radius_scale,
            material_bindings::material.min_point_size,
            material_bindings::material.max_point_size
        );
    #endif

    #ifdef SIMPLE_MATERIAL_POINT_SIZE_WORLD
        radius = functions::compute_world_space_point_size(
            view_position,
            material_bindings::material.point_size,
            material_bindings::material.min_point_size,
            material_bindings::material.max_point_size
        );
    #endif

    #ifdef SIMPLE_MATERIAL_POINT_SIZE_LOCAL
        radius = functions::compute_world_space_point_size(
            view_position,
            material_bindings::material.point_size * radius_scale,
            material_bindings::material.min_point_size,
            material_bindings::material.max_point_size
        );
    #endif

    var world_vertex_position: vec3<f32>;

    #ifdef SIMPLE_MATERIAL_SHAPE_ORIENTATION_FACE_NORMAL
        var normal: vec3<f32>;
        #ifdef VERTEX_NORMALS
            normal = vertex.normal;
        #else
            normal = material_bindings::material.default_normal;
        #endif
        #ifdef VERTEX_TANGENTS
            world_vertex_position = functions::compute_world_vertex_position_oriented_with_tangent(
                world_position.xyz,
                normal,
                vertex.tangent.xyz,
                world_from_local,
                shape.position,
                radius
            );
        #else
            world_vertex_position = functions::compute_world_vertex_position_oriented(
                world_position.xyz,
                normal,
                world_from_local,
                shape.position,
                radius
            );
        #endif

        view_vertex_position = position_world_to_view(world_vertex_position);
    #else // case billboard
        view_vertex_position = functions::compute_view_billboard_vertex_position(
            view_position,
            shape.position,
            radius
        );

        world_vertex_position = position_view_to_world(view_vertex_position, view.world_from_view);
    #endif


    out.position = position_view_to_clip(view_vertex_position);

    // send also the raw world position (not interpolated) for material computations (eg: gradients)
    out.world_position = world_position;

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
        let local_from_world = pointcloud_functions::get_local_from_world();

        // compute the radius in local space
        let radius_vector_world = vec3<f32>(radius, 0.0, 0.0);
        let radius_vector_local = local_from_world * vec4<f32>(radius_vector_world.xyz, 0.0);
        let radius_local = length(radius_vector_local) / 2.0;

        var shape_uv = shape.uv;

        // If we're using billboards, we might need to fix the flip effect when
        // the viewer go to the otherside, or up side down.
        // It can be fixed if we know the tangents & bitangent.
        // We try best to worst case, always giving priority to the most details.
        // Here the different cases in priority order:
        // 1. We know the normal and the tangent, we then compute the bitangent.
        // 2. We know only the normal, we assume `uv_u` material parameter is the tangent.
        // 3. We have no normals, we assume `uv_u` is the tangent, and `uv_v` the bitangent.
        #ifndef SIMPLE_MATERIAL_SHAPE_ORIENTATION_FACE_NORMAL
            // we need to know the local up/right vectors
            let view_right = vec3<f32>(1.0, 0.0, 0.0);
            let view_up    = vec3<f32>(0.0, 1.0, 0.0);

            let local_right = (local_from_world * (view.world_from_view * vec4<f32>(view_right, 0.0))).xyz;
            let local_up    = (local_from_world * (view.world_from_view * vec4<f32>(view_up, 0.0))).xyz;

            var tangent: vec3<f32>;
            var bitangent: vec3<f32>;

            #ifdef VERTEX_NORMALS
                #ifdef VERTEX_TANGENTS
                    tangent = vertex.tangent.xyz;
                #else // not VERTEX_TANGENTS
                    tangent = material_bindings::material.uv_u;
                #endif // VERTEX_TANGENTS

                bitangent = cross(vertex.normal, tangent);
            #else // not VERTEX_NORMALS
                    tangent = material_bindings::material.uv_u;
                    bitangent = material_bindings::material.uv_v;
            #endif // VERTEX_NORMALS

            // check if a flip is needed
            let flip_u = dot(tangent, local_right) < 0.0;
            let flip_v = dot(bitangent, local_up) < 0.0;

            // apply the flip
            shape_uv.x = select(shape_uv.x, 1.0 - shape_uv.x, flip_u);
            shape_uv.y = select(shape_uv.y, 1.0 - shape_uv.y, flip_v);
        #endif // SIMPLE_MATERIAL_SHAPE_ORIENTATION_FACE_NORMAL

        // compute final UV for this vertex
        base_uv = base_uv + (shape_uv - vec2<f32>(0.5, 0.5)) * radius_local;
    #endif

    #ifdef SIMPLE_MATERIAL_UV_MAPPING_PLANAR
        // compute the vertex position in object space
        let local_from_world = pointcloud_functions::get_local_from_world();
        let local_vertex_position = local_from_world * vec4<f32>(world_vertex_position, 1.0);

        var aabb_size = pointcloud.aabb_max.xyz - pointcloud.aabb_min.xyz;

        // Prevent zeros and NaN after division (sometimes it works without it, sometimes not)
        // Commented because another technique is used below
        // aabb_size = max(aabb_size, vec3<f32>(0.00001));

        var vertex_position_normalized = (local_vertex_position.xyz - pointcloud.aabb_min.xyz) / aabb_size;

        // Remove potential NaN values, setting UV a center in this special case
        vertex_position_normalized.x = select(vertex_position_normalized.x, 0.5, aabb_size.x == 0);
        vertex_position_normalized.y = select(vertex_position_normalized.y, 0.5, aabb_size.y == 0);
        vertex_position_normalized.z = select(vertex_position_normalized.z, 0.5, aabb_size.z == 0);

        let half_size_uv = (radius / radius_scale / 2.0) / aabb_size;

        let axis_u = material_bindings::material.uv_u;
        let axis_v = material_bindings::material.uv_v;

        base_uv = vec2<f32>(
            dot(axis_u, vertex_position_normalized),
            1.0 - dot(axis_v, vertex_position_normalized),
        );
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
