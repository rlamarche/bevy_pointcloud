#define_import_path bevy_pointcloud::materials::simple::binding

struct PointCloudMaterial {
    point_size: f32,
    min_point_size: f32,
    max_point_size: f32,
#ifdef SIXTEEN_BYTE_ALIGNMENT
    // WebGL2 structs must be 16 byte aligned.
    _webgl2_padding: f32,
#endif
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> material: PointCloudMaterial;
