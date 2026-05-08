use std::{cmp::Reverse, marker::PhantomData};

use bevy_ecs::{resource::Resource, world::FromWorld};
use bevy_platform::collections::HashMap;
use offset_allocator::{Allocation, Allocator};
use ordered_float::OrderedFloat;
use priority_queue::PriorityQueue;

use crate::octree::{
    extract::{render::buffer::RenderNodeData, OctreeNodeExtraction},
    node::{NodeData, OctreeNodeKey},
};

#[derive(Resource)]
pub struct OctreeBufferSettings<E: OctreeNodeExtraction> {
    pub(crate) max_size: usize,
    pub(crate) _phantom: PhantomData<fn() -> E>,
}

pub struct NodeAllocation<T: NodeData> {
    pub octree_node_key: OctreeNodeKey<T>,
    pub(crate) allocation: Allocation,
    pub start: u32,
    pub count: u32,
}

impl<T: NodeData> Clone for NodeAllocation<T> {
    fn clone(&self) -> Self {
        Self {
            octree_node_key: self.octree_node_key.clone(),
            allocation: self.allocation.clone(),
            start: self.start.clone(),
            count: self.count.clone(),
        }
    }
}

#[derive(Resource)]
pub struct OctreeNodeAllocations<E: OctreeNodeExtraction> {
    pub(crate) allocator: Allocator,
    pub(crate) max_instances: u32,
    pub(crate) allocations: HashMap<OctreeNodeKey<E::NodeData>, NodeAllocation<E::NodeData>>,
    pub(crate) freed_nodes_this_frame: Vec<NodeAllocation<E::NodeData>>,
    pub(crate) allocated_nodes_this_frame: Vec<NodeAllocation<E::NodeData>>,
    _phantom: PhantomData<fn() -> E>,
}

impl<E: OctreeNodeExtraction> FromWorld for OctreeNodeAllocations<E> {
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        let settings = world.resource::<OctreeBufferSettings<E>>();

        // compute the maximum number of instances
        let max_instances = (settings.max_size
            / std::mem::size_of::<<E::ExtractedNodeData as RenderNodeData>::InstanceData>())
            as u32;

        Self {
            allocator: Allocator::new(max_instances),
            max_instances,
            allocations: HashMap::new(),
            freed_nodes_this_frame: Vec::new(),
            allocated_nodes_this_frame: Vec::new(),
            _phantom: PhantomData,
        }
    }
}

/// This resource contains a priority queue to determine which nodes to evict first.
/// Nodes that are seen less recently are first in this queue.
#[derive(Resource)]
pub struct ExtractOctreeNodeEvictionQueue<E: OctreeNodeExtraction> {
    pub eviction_queue:
        PriorityQueue<OctreeNodeKey<E::NodeData>, Reverse<OctreeNodeEvictionPriority>>,
}

impl<E: OctreeNodeExtraction> Default for ExtractOctreeNodeEvictionQueue<E> {
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

// impl PartialOrd for OctreeNodeEvictionPriority {
//     fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
//         match self.timestamp.partial_cmp(&other.timestamp) {
//             Some(core::cmp::Ordering::Equal) => {}
//             ord => return ord,
//         }
//         self.weight.partial_cmp(&other.weight)
//     }
// }
