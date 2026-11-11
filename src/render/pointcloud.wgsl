#import bevy_pbr::{
    view_transformations::{
        position_world_to_view,
        position_view_to_clip,
    }
}

#import bevy_pointcloud::{
    forward_io::{ShapeInput, InstanceInput, VertexOutput},
    pointcloud_bindings::pointcloud,
    pointcloud_functions,
}


@vertex
fn vertex(
    shape: ShapeInput,
    vertex: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    // Because we are not doing batching, the instance index is always 0.
    out.instance_index = 0;

    // We assume VERTEX_POSITIONS & SHAPE_POSITIONS are set.

    let world_from_local = pointcloud_functions::get_world_from_local();

    let point_world_position =
        pointcloud_functions::mesh_position_local_to_world(
            world_from_local,
            vec4<f32>(vertex.position, 1.0)
        ).xyz;

    let point_view_position = position_world_to_view(point_world_position);
    #ifdef POINTCLOUD_NEEDS_POINT_VIEW_POSITION
        out.point_view_position = point_view_position;
    #endif

    let radius = pointcloud_functions::compute_point_radius(
        vertex.position,
        world_from_local,
        point_view_position
    );

    // Get splat normal (fallback to default)
    #ifdef SHAPE_NORMALS
        let shape_normal = shape.normal;
    #else
        let shape_normal = pointcloud.default_normal.xyz;
    #endif

    // Get point normal (fallback to default)
    #ifdef INSTANCE_NORMALS
        let normal = vertex.normal;
    #else
        let normal = pointcloud.default_normal.xyz;
    #endif

    // Prepare tangent
    #ifdef VERTEX_TANGENTS
        let tangent = vertex.tangent.xyz;
    #else
        let tangent = vec3<f32>(0.0);
    #endif

    // Compute vertex positions
    #ifdef POINTCLOUD_NEEDS_NORMAL
        var pos = pointcloud_functions::compute_point_vertex_positions_normal(
            point_view_position,
            point_world_position,
            world_from_local,
            shape.position,
            shape_normal,
            radius,
            normal,
            tangent,
        );
    #else
        var pos = pointcloud_functions::compute_point_vertex_positions(
            point_view_position,
            point_world_position,
            world_from_local,
            shape.position,
            radius,
            normal,
            tangent,
        );
    #endif

    out.position = pos.clip_position;
    out.view_position = pos.view_position;

    out.radius = radius;
    #ifdef POINTCLOUD_NEEDS_LOG_DEPTH
        out.log_depth = log2(-point_view_position.z);
    #endif

    // TODO move this in simple material
    #ifdef WEIGHTED_SPLATS_ENABLED
        #ifdef POINTCLOUD_PASS_DEPTH
    		let original_depth = pos.clip_position.w;

            if (original_depth > 0.0) {
                let adjust = 1.0 + (2.0 * radius) / original_depth;

                pos.view_position *= adjust;
                out.position = position_view_to_clip(pos.view_position);
            }
        #endif // POINTCLOUD_PASS_DEPTH
	#endif // WEIGHTED_SPLATS_ENABLED

	#ifdef POINTCLOUD_NEEDS_WORLD_POSITION
        out.world_position = vec4<f32>(pos.world_position, 1.0);
    #endif
    #ifdef POINTCLOUD_NEEDS_NORMAL
        out.world_normal = pos.world_normal;
    #endif

    #ifdef VERTEX_COLORS
        out.color = vertex.color;
    #endif

    #ifdef INSTANCE_UVS_B
        out.uv_b = vertex.uv_b;
    #endif

    #ifdef VERTEX_TANGENTS
        out.world_tangent = pointcloud_functions::mesh_tangent_local_to_world(
            world_from_local,
            vertex.tangent,
        );
    #endif

    #ifdef INSTANCE_UVS_A
        let vertex_uv = vertex.uv;
    #else
        let vertex_uv = vec2<f32>(0.0);
    #endif

    #ifdef POINTCLOUD_NEEDS_UVS
        out.uv = pointcloud_functions::compute_point_uv(
            vertex_uv,
            shape.uv,
            shape_normal,
            tangent,
            pos.world_position,
            radius,
        );
    #endif

    #ifdef SHAPE_UVS_A
        out.shape_uv = shape.uv;
    #endif // SHAPE_UVS_A

    return out;
}
