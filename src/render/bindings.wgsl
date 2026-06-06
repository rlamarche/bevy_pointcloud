#define_import_path bevy_pointcloud::bindings

@group(1) @binding(0)
var<uniform> world_from_local: mat4x4<f32>;


#ifdef IS_OCTREE

@group(3) @binding(0)
var visible_nodes: texture_2d<u32>;

@group(4) @binding(0)
var<uniform> octree_node: OctreeNode;

@group(5) @binding(0)
var<uniform> octree_entity: OctreeEntity;

#endif
