#define_import_path bevy_pointcloud::minimal_material_bindings

#import bevy_pointcloud::minimal_material_types::MinimalPointCloudMaterial

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: MinimalPointCloudMaterial;
