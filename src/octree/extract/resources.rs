use std::{cmp::Reverse, marker::PhantomData};

use bevy_ecs::prelude::*;
use bevy_platform::collections::HashMap;
use bevy_render::sync_world::RenderEntity;
use offset_allocator::{Allocation, Allocator};
use ordered_float::OrderedFloat;
use priority_queue::PriorityQueue;

use crate::octree::node::{NodeData, OctreeNodeKey};

#[derive(Resource)]
pub struct OctreeBufferSettings<T: NodeData> {
    pub(crate) stride: usize,
    pub(crate) max_size: usize,
    pub(crate) _phantom: PhantomData<fn() -> T>,
}

pub struct NodeAllocation<T: NodeData> {
    pub octree_node_key: OctreeNodeKey<T>,
    pub allocation: Allocation,
    pub offset: u64,
    pub size: u64,
    pub start: u32,
    pub count: u32,
}

impl<T: NodeData> Clone for NodeAllocation<T> {
    fn clone(&self) -> Self {
        Self {
            octree_node_key: self.octree_node_key.clone(),
            allocation: self.allocation,
            offset: self.offset,
            size: self.size,
            start: self.start,
            count: self.count,
        }
    }
}

#[derive(Resource)]
pub struct OctreeNodeAllocations<T: NodeData> {
    pub(crate) allocator: Allocator,
    pub max_instances: u32,
    pub(crate) buffer_size: u64,
    pub(crate) allocations: HashMap<OctreeNodeKey<T>, NodeAllocation<T>>,
    pub(crate) removed_octrees_this_frame: Vec<(Entity, RenderEntity)>,
    pub(crate) freed_nodes_this_frame: Vec<NodeAllocation<T>>,
    pub(crate) allocated_nodes_this_frame: Vec<NodeAllocation<T>>,
    _phantom: PhantomData<fn() -> T>,
}

impl<T: NodeData> FromWorld for OctreeNodeAllocations<T> {
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        let settings = world.resource::<OctreeBufferSettings<T>>();

        // compute the maximum number of instances
        let max_instances = (settings.max_size / settings.stride) as u32;

        Self {
            allocator: Allocator::new(max_instances),
            max_instances,
            buffer_size: settings.max_size as u64,
            allocations: HashMap::new(),
            removed_octrees_this_frame: Vec::new(),
            freed_nodes_this_frame: Vec::new(),
            allocated_nodes_this_frame: Vec::new(),
            _phantom: PhantomData,
        }
    }
}

/// This resource contains a priority queue to determine which nodes to evict first.
/// Nodes that are seen less recently are first in this queue.
#[derive(Resource)]
pub struct ExtractOctreeNodeEvictionQueue<T: NodeData> {
    pub eviction_queue: PriorityQueue<OctreeNodeKey<T>, Reverse<OctreeNodeEvictionPriority>>,
}

impl<T: NodeData> Default for ExtractOctreeNodeEvictionQueue<T> {
    fn default() -> Self {
        Self {
            eviction_queue: PriorityQueue::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub struct OctreeNodeEvictionPriority {
    pub elapsed: u128,
    pub weight: OrderedFloat<f32>,
}
