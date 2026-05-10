use std::cmp::Reverse;

use bevy_ecs::prelude::*;
use bevy_time::{Real, Time};

use super::resources::ExtractOctreeNodeEvictionQueue;
use crate::octree::{
    extract::{
        resources::{OctreeNodeAllocations, OctreeNodeEvictionPriority},
        OctreeNodeExtraction,
    },
    visibility::resources::GlobalVisibleOctreeNodes,
};

/// This system update the octree node eviction queue with latest informations
pub fn update_extract_octree_node_eviction_queue<E: OctreeNodeExtraction>(
    mut octree_node_eviction_queue: ResMut<ExtractOctreeNodeEvictionQueue<E>>,
    octree_buffer_allocator: Res<OctreeNodeAllocations<E>>,
    global_visible_octree_nodes: Res<GlobalVisibleOctreeNodes<E::NodeData>>,
    time: Res<Time<Real>>,
) {
    let elapsed = time.elapsed().as_millis();
    let eviction_queue = &mut octree_node_eviction_queue.eviction_queue;

    for (key, &weight) in &global_visible_octree_nodes.visible_octree_nodes {
        if octree_buffer_allocator.allocations.contains_key(key) {
            eviction_queue.push(
                key.clone(),
                Reverse(OctreeNodeEvictionPriority { elapsed, weight }),
            );
        }
    }
}
