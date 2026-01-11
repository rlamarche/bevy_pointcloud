use crate::octree::asset::Octree;
use crate::octree::node::{NodeData, OctreeNode, OctreeNodeKey};
use crate::octree::storage::NodeId;
use bevy_asset::AssetId;
use bevy_ecs::prelude::*;
use bevy_platform::collections::HashSet;
use priority_queue::PriorityQueue;
use std::cmp::Reverse;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// This resource contains all visible octree nodes in the current iteration, across all cameras
#[derive(Resource)]
pub struct GlobalVisibleOctreeNodes<T: NodeData, C: Component> {
    pub visible_octree_nodes: HashSet<OctreeNodeKey<T>>,
    phantom: PhantomData<C>,
}

impl<T: NodeData, C: Component> Default for GlobalVisibleOctreeNodes<T, C> {
    fn default() -> Self {
        Self {
            visible_octree_nodes: HashSet::new(),
            phantom: PhantomData,
        }
    }
}

impl<T: NodeData, C: Component> GlobalVisibleOctreeNodes<T, C> {
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
