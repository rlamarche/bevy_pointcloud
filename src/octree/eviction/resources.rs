use crate::octree::node::{NodeData, OctreeNodeKey};
use bevy_ecs::prelude::*;
use priority_queue::PriorityQueue;
use std::cmp::Reverse;
use std::marker::PhantomData;

/// This resource contains a priority queue to determine which nodes to evict first.
/// Nodes that are seen less recently are first in this queue.
#[derive(Resource)]
pub struct OctreeNodeEvictionQueue<T: NodeData> {
    pub eviction_queue: PriorityQueue<OctreeNodeKey<T>, Reverse<u128>>,
}

impl<T: NodeData> Default for OctreeNodeEvictionQueue<T> {
    fn default() -> Self {
        Self {
            eviction_queue: PriorityQueue::new(),
        }
    }
}

#[derive(Resource)]
pub struct OctreeNodeEvictionSettings<T: NodeData> {
    pub max_size: usize,
    pub(crate) phantom_data: PhantomData<fn () -> T>,
}
