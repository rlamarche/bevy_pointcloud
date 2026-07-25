#define_import_path bevy_pointcloud::simple_material_bindings

#import bevy_pointcloud::simple_material_types::SimplePointCloudMaterial

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: SimplePointCloudMaterial;

@group(#{MATERIAL_BIND_GROUP}) @binding(1) var base_color_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var base_color_sampler: sampler;
