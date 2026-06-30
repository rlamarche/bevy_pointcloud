#define_import_path bevy_pointcloud::pointcloud_bindings

#import bevy_pointcloud::pointcloud_types::PointCloud

@group(2) @binding(0) var<uniform> pointcloud: PointCloud;
