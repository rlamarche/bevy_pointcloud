#import bevy_render::view::position_view_to_world;

#import bevy_pbr::{
    mesh_view_bindings::view,
    mesh_functions::mesh_position_local_to_world,
    view_transformations::{
        position_world_to_clip,
        position_world_to_view,
        position_view_to_clip,
    },
}

#import bevy_pbr::forward_io::VertexOutput;

#import bevy_pointcloud::{
    forward_io::{ShapeInput, InstanceInput},
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

    out.instance_index = 0;

    // We assume VERTEX_POSITIONS & SHAPE_POSITIONS are set.

    // out.instance_position = vertex.position;

    #ifdef VERTEX_NORMALS
        out.world_normal = pointcloud_functions::mesh_normal_local_to_world(vertex.normal);
    #else
        // normal facing camera because we do billboarding when no normals
        out.world_normal = normalize(-view.world_from_view[2].xyz);
    #endif

    let world_from_local = pointcloud_functions::get_world_from_local();

    // compute the world position of point coordinates
    let world_position = mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    var view_vertex_position: vec3<f32>;

    let view_position = position_world_to_view(world_position.xyz);

    var radius: f32 = 0.5;
    let radius_scale = functions::extract_max_scale(world_from_local);

    var world_vertex_position: vec3<f32>;

    #ifdef VERTEX_NORMALS
        let normal: vec3<f32> = vertex.normal;

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
    out.world_position = vec4<f32>(world_vertex_position, 1.0);

    #ifdef VERTEX_COLORS
        out.color = vertex.color;
    #endif

    #ifdef VERTEX_UVS_A
        out.uv = vertex.uv;
    #endif
    #ifdef VERTEX_UVS_B
        out.uv_b = vertex.uv_b;
    #endif

    return out;
}
