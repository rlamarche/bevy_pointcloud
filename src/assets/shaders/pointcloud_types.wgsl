#define_import_path bevy_pointcloud::pointcloud_types

struct PointCloud {
    aabb_min: vec4<f32>,
    aabb_max: vec4<f32>,
    // Affine 4x3 matrices transposed to 3x4
    // Use bevy_render::maths::affine3_to_square to unpack
    world_from_local: mat3x4<f32>,
    previous_world_from_local: mat3x4<f32>,
    // 3x3 matrix packed in mat2x4 and f32 as:
    // [0].xyz, [1].x,
    // [1].yz, [2].xy
    // [2].z
    // Use bevy_pbr::mesh_functions::mat2x4_f32_to_mat3x3_unpack to unpack
    local_from_world_transpose_a: mat2x4<f32>,
    local_from_world_transpose_b: f32,
    material_bind_group_slot: u32,
    spacing: f32,

    // Splat settings
    point_size: f32,
    min_point_size: f32,
    max_point_size: f32,
    radius: f32,

    // Vectors padded to vec4<f32> for std140 layout alignment
    default_normal: vec4<f32>,
    uv_u: vec4<f32>,
    uv_v: vec4<f32>,

    // Affine 2D transformation matrix packed as two vec4<f32>:
    // [0].xy = col[0], [0].zw = col[1]
    // [1].xy = translation
    // Unpack with mat2x2<f32>(uv_transform_a.xy, uv_transform_a.zw)
    uv_transform_a: vec4<f32>,
    uv_transform_b: vec4<f32>,
};

struct PointVertexPositions {
    world_position: vec3<f32>,
    view_position: vec3<f32>,
    clip_position: vec4<f32>,
};

struct PointVertexPositionsNormal {
    world_position: vec3<f32>,
    view_position: vec3<f32>,
    clip_position: vec4<f32>,
    world_normal: vec3<f32>,
};
