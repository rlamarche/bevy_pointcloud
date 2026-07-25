#import bevy_render::view::position_view_to_world;

#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_view,
}

#import bevy_pbr::forward_io::VertexOutput;

#import bevy_pointcloud::{
    forward_io::{ShapeInput, InstanceInput},
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

    // TODO remove world_from_local from PointCloudUniform and use this above
    // let world_from_local = mesh_functions::get_world_from_local(out.instance_index);

    let point_world_position =
        mesh_functions::mesh_position_local_to_world(
            world_from_local,
            vec4<f32>(vertex.position, 1.0)
        ).xyz;

    let point_view_position = position_world_to_view(point_world_position);

    let radius = pointcloud_functions::compute_point_radius(
        world_from_local,
        point_view_position
    );

    // Prepare normal (fallback to default)
    #ifdef VERTEX_NORMALS
        let normal = vertex.normal;
    #else
        let normal = pointcloud.default_normal;
    #endif

    // Prepare tangent
    #ifdef VERTEX_TANGENTS
        let tangent = vertex.tangent.xyz;
    #else
        let tangent = vec3<f32>(0.0);
    #endif

    // Compute vertex positions
    let pos = pointcloud_functions::compute_point_vertex_positions(
        point_world_position,
        world_from_local,
        shape.position,
        radius,
        normal,
        tangent,
    );

    out.position = pos.clip_position;
    out.world_position = vec4<f32>(pos.world_position, 1.0);
    out.world_normal = mesh_functions::mesh_normal_local_to_world(normal, out.instance_index);


    #ifdef VERTEX_COLORS
        out.color = vertex.color;
    #endif

    #ifdef VERTEX_UVS_A
        out.uv = vertex.uv;
    #endif
    #ifdef VERTEX_UVS_B
        out.uv_b = vertex.uv_b;
    #endif

    #ifdef VERTEX_TANGENTS
        out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
            world_from_local,
            vertex.tangent,
            out.instance_index,
        );
    #endif

    #ifdef VERTEX_UVS
        let vertex_uv = vertex.uv;
    #else
        let vertex_uv = vec2<f32>(0.0);
    #endif

    out.uv = pointcloud_functions::compute_point_uv(
        vertex_uv,
        shape.uv,
        normal,
        tangent,
        pos.world_position,
        radius,
    );

    return out;
}
