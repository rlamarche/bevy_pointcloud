use std::cmp::Reverse;

use bevy_time::{Real, Time};

use super::resources::OctreeServerEvictionQueue;
use crate::{
    bevy::prelude::*,
    octree::{
        asset::Octree, node::NodeData, server::resources::OctreeServerSettings,
        visibility::resources::GlobalVisibleOctreeNodes, OctreeTotalSize,
    },
};

/// This system update the octree node eviction queue with latest informations
pub fn update_octree_server_node_eviction_queue<T: NodeData>(
    mut octree_node_eviction_queue: ResMut<OctreeServerEvictionQueue<T>>,
    global_visible_octree_nodes: Res<GlobalVisibleOctreeNodes<T>>,
    time: Res<Time<Real>>,
) {
    let elapsed = time.elapsed().as_millis();
    let eviction_queue = &mut octree_node_eviction_queue.eviction_queue;

    for (key, _) in &global_visible_octree_nodes.visible_octree_nodes {
        eviction_queue.push(key.clone(), Reverse(elapsed));
    }
}

/// This system remove nodes data to meet memory budget requirements, and update octree node total size
pub fn evict_octree_nodes<T: NodeData>(
    settings: Res<OctreeServerSettings<T>>,
    mut octree_node_eviction_queue: ResMut<OctreeServerEvictionQueue<T>>,
    global_visible_octree_nodes: Res<GlobalVisibleOctreeNodes<T>>,
    mut octree_total_size: ResMut<OctreeTotalSize<T>>,
    mut octrees: ResMut<Assets<Octree<T>>>,
) {
    let eviction_queue = &mut octree_node_eviction_queue.eviction_queue;

    let total_size = &mut octree_total_size.total_size;

    while *total_size > settings.max_size {
        if let Some((key, _)) = eviction_queue.pop_if(|key, _| {
            !global_visible_octree_nodes
                .visible_octree_nodes
                .contains_key(key)
        }) {
            let Some(octree) = octrees.get_mut(key.octree_id) else {
                continue;
            };

            if let Some(data) = octree.remove_node_data(key.node_id) {
                *total_size -= data.size();
            }
        } else {
            break;
        }
    }
}
