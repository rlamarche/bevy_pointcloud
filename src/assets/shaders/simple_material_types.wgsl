#define_import_path bevy_pointcloud::simple_material_types

struct SimplePointCloudMaterial {
    base_color: vec4<f32>,
    point_size: f32,
    shape_radius: f32,
    // 'flags' is a bit field indicating various options. u32 is 32 bits so we have up to 32 options.
    flags: u32,
}

const SIMPLE_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT: u32            = 1u << 0u;
