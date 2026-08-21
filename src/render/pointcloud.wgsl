#import bevy_pbr::{
    view_transformations::position_world_to_view,
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
    let pos = pointcloud_functions::compute_point_vertex_positions_normal(
        point_world_position,
        world_from_local,
        shape.position,
        shape_normal,
        radius,
        normal,
        tangent,
    );

    out.position = pos.clip_position;
    out.world_position = vec4<f32>(pos.world_position, 1.0);
    out.world_normal = pos.world_normal;

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

    out.uv = pointcloud_functions::compute_point_uv(
        vertex_uv,
        shape.uv,
        shape_normal,
        tangent,
        pos.world_position,
        radius,
    );

    #ifdef SHAPE_UVS_A
        out.shape_uv = shape.uv;
    #endif // SHAPE_UVS_A

    return out;
}
