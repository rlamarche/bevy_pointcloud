import package::render::pointcloud_types::PointCloud;

@group(constants::POINTCLOUD_BIND_GROUP) @binding(1)
var<uniform> pointcloud: PointCloud;

@group(constants::POINTCLOUD_BIND_GROUP) @binding(2)
var visible_nodes: texture_2d<u32>;
