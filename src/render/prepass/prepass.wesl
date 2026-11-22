import bevy_pbr::render::{
    morph,
    prepass_bindings,
    skinning,
    mesh_view_bindings::view,
    view_transformations::{position_world_to_prev_view, position_world_to_view}
};

@if(PREPASS_FRAGMENT)
import bevy_pbr::prepass::io::FragmentOutput;

import package::render::{
    functions,
    pointcloud_functions,
    pointcloud_bindings::pointcloud,
    prepass::prepass_io::{InstanceInput, ShapeInput, VertexOutput}
};

@if(DEFERRED_PREPASS)
import bevy_pbr::render::rgb9e5;

@vertex
fn vertex(shape: ShapeInput, vertex: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    out.instance_index = 0;

    // We assume VERTEX_POSITIONS & SHAPE_POSITIONS are set.

    let world_from_local = pointcloud_functions::get_world_from_local();

    let point_world_position = pointcloud_functions::mesh_position_local_to_world(
            world_from_local,
            vec4<f32>(vertex.position, 1.0),
        ).xyz;

    let point_view_position = position_world_to_view(point_world_position);

    let radius = pointcloud_functions::compute_point_radius(
            vertex.position,
            world_from_local,
            point_view_position,
        );

    var shape_normal: vec3<f32>;
    @if(NORMAL_PREPASS_OR_DEFERRED_PREPASS) {
        // Get splat normal (fallback to default)
        @if(SHAPE_NORMALS) {
            shape_normal = shape.normal;
        } @else {
            shape_normal = pointcloud.default_normal.xyz;
        }
    }

    // Get point normal (fallback to default)
    var normal: vec3<f32>;
    @if(INSTANCE_NORMALS) {
        normal = vertex.normal;
    } @else {
        normal = pointcloud.default_normal.xyz;
    }

    // Prepare tangent
    var tangent: vec3<f32>;
    @if(VERTEX_TANGENTS) {
        tangent = vertex.tangent.xyz;
    } @else {
        tangent = vec3<f32>(0.0);
    }

    // Compute vertex positions (extraction des champs communs dans des variables typées)
    var clip_position: vec4<f32>;
    var world_position: vec3<f32>;
    var world_normal: vec3<f32>;

    @if(NORMAL_PREPASS_OR_DEFERRED_PREPASS) {
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
        world_position = pos.world_position;
        world_normal = pos.world_normal;
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
        world_position = pos.world_position;
    }

    out.position = clip_position;
    out.world_position = vec4<f32>(world_position, 1.0);

    @if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
        out.unclipped_depth = out.position.z;
        out.position.z = min(out.position.z, 1.0); // Clamp depth to avoid clipping
    }

    @if(INSTANCE_UVS_A) {
        out.uv = vertex.uv;
    }

    @if(INSTANCE_UVS_B) {
        out.uv_b = vertex.uv_b;
    }

    @if(NORMAL_PREPASS_OR_DEFERRED_PREPASS) {
        @if(VERTEX_NORMALS) {
            out.world_normal = world_normal;
        }

        @if(VERTEX_TANGENTS) {
            out.world_tangent = pointcloud_functions::mesh_tangent_local_to_world(
                    world_from_local,
                    vertex.tangent,
                );
        }
    }

    @if(VERTEX_COLORS) {
        out.color = vertex.color;
    }

    // Compute the motion vector for TAA among other purposes. For this we need
    // to know where the vertex was last frame.
    @if(MOTION_VECTOR_PREPASS) {
        let prev_vertex = vertex;

        // Use vertex_no_morph.instance_index instead of prev_vertex.instance_index to work around a wgpu dx12 bug.
        // See https://github.com/gfx-rs/naga/issues/2416
        let prev_model = pointcloud_functions::get_previous_world_from_local();

        let prev_point_world_position = pointcloud_functions::mesh_position_local_to_world(
                prev_model,
                vec4<f32>(prev_vertex.position, 1.0),
            ).xyz;

        let prev_point_view_position = position_world_to_prev_view(prev_point_world_position);

        var prev_world_position: vec3<f32>;

        // Compute previous vertex positions
        @if(NORMAL_PREPASS_OR_DEFERRED_PREPASS) {
            let prev_pos = pointcloud_functions::compute_point_vertex_positions_normal(
                    prev_point_view_position,
                    prev_point_world_position,
                    prev_model,
                    shape.position,
                    shape_normal,
                    radius,
                    normal,
                    tangent,
                );
            prev_world_position = prev_pos.world_position;
        } @else {
            let prev_pos = pointcloud_functions::compute_point_vertex_positions(
                    prev_point_world_position,
                    prev_model,
                    shape.position,
                    radius,
                    normal,
                    tangent,
                );
            prev_world_position = prev_pos.world_position;
        }

        out.previous_world_position = vec4<f32>(prev_world_position, 1.0);
    }

    @if(VISIBILITY_RANGE_DITHER) {
        let mesh_world_from_local = pointcloud_functions::get_world_from_local();

        out.visibility_range_dither =
            pointcloud_functions::get_visibility_range_dither_level(mesh_world_from_local[3]);
    }

    @if(SHAPE_UVS_A) {
        out.shape_uv = shape.uv;
    }

    return out;
}

@if(PREPASS_FRAGMENT)
@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    @if(SPLAT_RADIUS) {
        // Perfect circle
        let diff = in.shape_uv - vec2<f32>(0.5, 0.5);
        let dist_sq = dot(diff, diff);
        let radius_sq = pointcloud.radius * pointcloud.radius;

        if dist_sq > radius_sq {
            discard;
        }
    }

    var out: FragmentOutput;

    @if(NORMAL_PREPASS) {
        out.normal = vec4(in.world_normal * 0.5 + vec3(0.5), 1.0);
    }

    @if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
        out.frag_depth = in.unclipped_depth;
    }

    @if(MOTION_VECTOR_PREPASS) {
        let clip_position_t = view.unjittered_clip_from_world * in.world_position;
        let clip_position = clip_position_t.xy / clip_position_t.w;
        let previous_clip_position_t = prepass_bindings::previous_view_uniforms.clip_from_world
            * in.previous_world_position;
        let previous_clip_position = previous_clip_position_t.xy / previous_clip_position_t.w;
        // These motion vectors are used as offsets to UV positions and are stored
        // in the range -1,1 to allow offsetting from the one corner to the
        // diagonally-opposite corner in UV coordinates, in either direction.
        // A difference between diagonally-opposite corners of clip space is in the
        // range -2,2, so this needs to be scaled by 0.5. And the V direction goes
        // down where clip space y goes up, so y needs to be flipped.
        out.motion_vector = (clip_position - previous_clip_position) * vec2(0.5, -0.5);
    }

    @if(DEFERRED_PREPASS) {
        // There isn't any material info available for this default prepass shader so we are just writing
        // emissive magenta out to the deferred gbuffer to be rendered by the first deferred lighting pass layer.
        // This is here so if the default prepass fragment is used for deferred magenta will be rendered, and also
        // as an example to show that a user could write to the deferred gbuffer if they were to start from this shader.
        out.deferred = vec4(0u, bevy_pbr::rgb9e5::vec3_to_rgb9e5_(vec3(1.0, 0.0, 1.0)), 0u, 0u);
        out.deferred_lighting_pass_id = 1u;
    }

    return out;
}

@if(!PREPASS_FRAGMENT)
@fragment
fn fragment(in: VertexOutput) {
    @if(SPLAT_RADIUS) {
        // Perfect circle
        let diff = in.shape_uv - vec2<f32>(0.5, 0.5);
        let dist_sq = dot(diff, diff);
        let radius_sq = pointcloud.radius * pointcloud.radius;

        if dist_sq > radius_sq {
            discard;
        }
    }
}
