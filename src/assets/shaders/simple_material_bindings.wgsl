#define_import_path bevy_pointcloud::simple_material_bindings

#import bevy_pointcloud::simple_material_types::SimplePointCloudMaterial

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: SimplePointCloudMaterial;
