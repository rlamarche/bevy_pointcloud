import package::materials::simple::simple_types::SimplePointCloudMaterial;

@group(constants::MATERIAL_BIND_GROUP) @binding(0)
var<uniform> material: SimplePointCloudMaterial;

@group(constants::MATERIAL_BIND_GROUP) @binding(1)
var base_color_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2)
var base_color_sampler: sampler;
