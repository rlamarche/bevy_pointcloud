struct PointCloudViewSettings {
    nb_clipping_planes: u32,
    nb_edl_neighbours: u32,
    edl_radius: f32,
    edl_strength: f32,
    clipping_planes: array<vec4<f32>, 8>,
    edl_neighbours: array<vec4<f32>, 8>,
}

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

const SIMPLE_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT: u32 = 1u << 0u;

const SIMPLE_MATERIAL_FLAGS_ALPHA_MODE_RESERVED_BITS: u32 = 7u << 29u; // (0b111u << 29u)
const SIMPLE_MATERIAL_FLAGS_ALPHA_MODE_OPAQUE: u32 = 0u << 29u;
const SIMPLE_MATERIAL_FLAGS_ALPHA_MODE_MASK: u32 = 1u << 29u;
const SIMPLE_MATERIAL_FLAGS_ALPHA_MODE_BLEND: u32 = 2u << 29u;
const SIMPLE_MATERIAL_FLAGS_ALPHA_MODE_PREMULTIPLIED: u32 = 3u << 29u;
const SIMPLE_MATERIAL_FLAGS_ALPHA_MODE_ADD: u32 = 4u << 29u;
const SIMPLE_MATERIAL_FLAGS_ALPHA_MODE_MULTIPLY: u32 = 5u << 29u;
const SIMPLE_MATERIAL_FLAGS_ALPHA_MODE_ALPHA_TO_COVERAGE: u32 = 6u << 29u;
