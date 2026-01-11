use crate::octree::asset::Octree;
use crate::octree::node::{NodeData, OctreeNodeKey};
use crate::octree::storage::NodeId;
use bevy_asset::AssetId;
use bevy_ecs::prelude::*;
use priority_queue::PriorityQueue;
use std::cmp::Reverse;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// This resource contains a priority queue to determine which nodes to evict first.
/// Nodes that are seen less recently are first in this queue.
#[derive(Resource)]
pub struct OctreeNodeEvictionQueue<T: NodeData, C: Component> {
    pub eviction_queue: PriorityQueue<OctreeNodeKey<T>, Reverse<u128>>,
    phantom: PhantomData<C>,
}

impl<T: NodeData, C: Component> Default for OctreeNodeEvictionQueue<T, C> {
    fn default() -> Self {
        Self {
            eviction_queue: PriorityQueue::new(),
            phantom: PhantomData,
        }
    }
}
