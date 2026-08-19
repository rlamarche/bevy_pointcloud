#define_import_path bevy_pointcloud::pointcloud_bindings

#import bevy_pointcloud::pointcloud_types::PointCloud

@group(3) @binding(0) var<uniform> pointcloud: PointCloud;

#ifdef IS_OCTREE

@group(4) @binding(0)
var visible_nodes: texture_2d<u32>;

#endif
