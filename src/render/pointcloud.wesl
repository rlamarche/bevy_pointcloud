import bevy_pbr::render::view_transformations::{position_view_to_clip, position_world_to_view};

import package::render::{
    pointcloud_functions,
    forward_io::{InstanceInput, ShapeInput, VertexOutput},
    pointcloud_bindings::pointcloud
};

@vertex
fn vertex(shape: ShapeInput, vertex: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    out.instance_index = 0;

    let world_from_local = pointcloud_functions::get_world_from_local();

    let point_world_position = pointcloud_functions::mesh_position_local_to_world(
            world_from_local,
            vec4<f32>(vertex.position, 1.0),
        ).xyz;

    let point_view_position = position_world_to_view(point_world_position);

    @if(POINTCLOUD_NEEDS_POINT_VIEW_POSITION) {
        out.point_view_position = point_view_position;
    }

    let radius = pointcloud_functions::compute_point_radius(
            vertex.position,
            world_from_local,
            point_view_position,
        );

    var shape_normal: vec3<f32>;
    @if(SHAPE_NORMALS) {
        shape_normal = shape.normal;
    } @else {
        shape_normal = pointcloud.default_normal.xyz;
    }

    var normal: vec3<f32>;
    @if(INSTANCE_NORMALS) {
        normal = vertex.normal;
    } @else {
        normal = pointcloud.default_normal.xyz;
    }

    var tangent: vec3<f32>;
    @if(VERTEX_TANGENTS) {
        tangent = vertex.tangent.xyz;
    } @else {
        tangent = vec3<f32>(0.0);
    }

    var clip_position: vec4<f32>;
    var view_position: vec3<f32>;
    var world_position: vec3<f32>;

    @if(POINTCLOUD_NEEDS_NORMAL) {
        let pos = pointcloud_functions::compute_point_vertex_positions_normal(
                point_view_position,
                point_world_position,
                world_from_local,
                shape.position,
                shape_normal,
                radius,
                normal,
                tangent,
            );
        clip_position = pos.clip_position;
        view_position = pos.view_position;
        world_position = pos.world_position;
        out.world_normal = pos.world_normal;
    } @else {
        let pos = pointcloud_functions::compute_point_vertex_positions(
                point_view_position,
                point_world_position,
                world_from_local,
                shape.position,
                radius,
                normal,
                tangent,
            );
        clip_position = pos.clip_position;
        view_position = pos.view_position;
        world_position = pos.world_position;
    }

    out.position = clip_position;
    out.view_position = view_position;
    out.radius = radius;

    @if(POINTCLOUD_NEEDS_LOG_DEPTH) {
        out.log_depth = log2(-point_view_position.z);
    }

    // TODO: move this in a custom vertex shader for each material
    @if(WEIGHTED_SPLATS_ENABLED && POINTCLOUD_PASS_DEPTH) {
        let original_depth = clip_position.w;

        if original_depth > 0.0 {
            let adjust = 1.0 + (2.0 * radius) / original_depth;

            view_position *= adjust;
            out.view_position = view_position;
            out.position = position_view_to_clip(view_position);
        }
    }

    @if(POINTCLOUD_NEEDS_WORLD_POSITION) {
        out.world_position = vec4<f32>(world_position, 1.0);
    }

    @if(VERTEX_COLORS) {
        out.color = vertex.color;
    }

    @if(INSTANCE_UVS_B) {
        out.uv_b = vertex.uv_b;
    }

    @if(VERTEX_TANGENTS) {
        out.world_tangent = pointcloud_functions::mesh_tangent_local_to_world(
                world_from_local,
                vertex.tangent,
            );
    }

    var vertex_uv: vec2<f32>;
    @if(INSTANCE_UVS_A) {
        vertex_uv = vertex.uv;
    } @else {
        vertex_uv = vec2<f32>(0.0);
    }

    @if(SHAPE_UVS_A) {
        out.shape_uv = shape.uv;
    } @else {
        out.shape_uv = shape.position.xy + vec2(0.5);
    }

    @if(POINTCLOUD_NEEDS_UVS) {
        out.uv = pointcloud_functions::compute_point_uv(
                vertex_uv,
                out.shape_uv,
                shape_normal,
                tangent,
                world_position,
                radius,
            );
    }

    return out;
}
