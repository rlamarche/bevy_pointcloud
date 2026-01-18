use crate::octree::asset::Octree;
use crate::octree::node::{NodeData, OctreeNode, OctreeNodeKey};
use bevy_asset::AssetId;
use bevy_ecs::prelude::*;
use bevy_platform::collections::HashSet;

/// This resource contains all visible octree nodes in the current iteration, across all cameras
#[derive(Resource)]
pub struct GlobalVisibleOctreeNodes<T: NodeData> {
    pub visible_octree_nodes: HashSet<OctreeNodeKey<T>>,
}

impl<T: NodeData> Default for GlobalVisibleOctreeNodes<T> {
    fn default() -> Self {
        Self {
            visible_octree_nodes: HashSet::new(),
        }
    }
}

impl<T: NodeData> GlobalVisibleOctreeNodes<T> {
    pub fn clear(&mut self) {
        self.visible_octree_nodes.clear();
    }

    pub fn add_visible_octree_node(&mut self, octree_id: AssetId<Octree<T>>, node: &OctreeNode<T>) {
        self.visible_octree_nodes.insert(OctreeNodeKey {
            octree_id,
            node_id: node.hierarchy.id,
        });
    }
}
