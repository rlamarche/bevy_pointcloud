#define_import_path bevy_pointcloud::simple_material_types

struct SimplePointCloudMaterial {
    base_color: vec4<f32>,
    end_color: vec4<f32>,
    gradient_direction: vec4<f32>,

    // Affine 2D transformation matrix packed as two vec4<f32>:
    // [0].xy = col[0], [0].zw = col[1]
    // [1].xy = translation
    // Unpack with mat2x2<f32>(uv_transform_a.xy, uv_transform_a.zw)
    uv_transform_a: vec4<f32>,
    uv_transform_b: vec4<f32>,

    color_stops: array<ColorStop, 8>,

    gradient_start: f32,
    gradient_end: f32,

    // 'flags' is a bit field indicating various options. u32 is 32 bits so we have up to 32 options.
    flags: u32,
}

struct ColorStop {
    color: vec4<f32>,
    point: f32,
}

const SIMPLE_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT: u32            = 1u << 0u;
