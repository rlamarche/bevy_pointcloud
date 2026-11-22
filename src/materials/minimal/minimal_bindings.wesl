import package::materials::minimal::minimal_types::MinimalPointCloudMaterial;

@group(constants::MATERIAL_BIND_GROUP) @binding(0)
var<uniform> material: MinimalPointCloudMaterial;
