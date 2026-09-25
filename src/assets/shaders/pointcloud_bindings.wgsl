#define_import_path bevy_pointcloud::pointcloud_bindings

#import bevy_pointcloud::pointcloud_types::PointCloud

@group(#{POINTCLOUD_BIND_GROUP}) @binding(1) var<uniform> pointcloud: PointCloud;

@group(#{POINTCLOUD_BIND_GROUP}) @binding(2) var visible_nodes: texture_2d<u32>;

#ifdef IS_OCTREE

#endif
