#define_import_path bevy_pointcloud::minimal_material_types

struct PointCloudViewSettings {
    nb_clipping_planes: u32,
    nb_edl_neighbours: u32,
    edl_radius: f32,
    edl_strength: f32,
    clipping_planes: array<vec4<f32>, 8>,
    edl_neighbours: array<vec4<f32>, 8>,
}

struct MinimalPointCloudMaterial {
    base_color: vec4<f32>,

    // 'flags' is a bit field indicating various options. u32 is 32 bits so we have up to 32 options.
    flags: u32,
}

struct ColorStop {
    color: vec4<f32>,
    point: f32,
}

const MINIMAL_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT: u32            = 1u << 0u;
