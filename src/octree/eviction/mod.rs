pub mod resources;

use crate::octree::OctreeTotalSize;
use crate::octree::asset::Octree;
use crate::octree::eviction::resources::OctreeNodeEvictionSettings;
use crate::octree::node::NodeData;
use crate::octree::visibility::CheckOctreeNodesVisibility;
use crate::octree::visibility::resources::GlobalVisibleOctreeNodes;
use bevy_app::{App, Plugin, PostUpdate};
use bevy_asset::Assets;
use bevy_ecs::prelude::*;
use bevy_time::{Real, Time};
use resources::OctreeNodeEvictionQueue;
use std::cmp::Reverse;
use std::marker::PhantomData;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct OctreeNodesEviction;

#[derive(Clone, Resource)]
pub struct OctreeEvictionPlugin<T> {
    pub max_size: usize,
    phantom: PhantomData<fn() -> T>,
}

impl<T> OctreeEvictionPlugin<T> {
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            max_size,
            phantom: PhantomData,
        }
    }
}

impl<T> Default for OctreeEvictionPlugin<T> {
    fn default() -> Self {
        OctreeEvictionPlugin {
            max_size: 32 * 10 * 1024 * 1024, // 10 millions points max
            phantom: PhantomData,
        }
    }
}
impl<T> Plugin for OctreeEvictionPlugin<T>
where
    T: NodeData,
{
    fn build(&self, app: &mut App) {
        app.insert_resource(OctreeNodeEvictionSettings::<T> {
            max_size: self.max_size,
            phantom_data: PhantomData,
        })
        .init_resource::<OctreeNodeEvictionQueue<T>>()
        .add_systems(
            PostUpdate,
            (
                update_octree_node_eviction_queue::<T>,
                evict_octree_nodes::<T>.after(update_octree_node_eviction_queue::<T>),
            )
                .in_set(OctreeNodesEviction),
        )
        .configure_sets(
            PostUpdate,
            (CheckOctreeNodesVisibility, OctreeNodesEviction).chain(),
        );
    }
}

/// This system update the octree node eviction queue with latest informations
pub fn update_octree_node_eviction_queue<T: NodeData>(
    mut octree_node_eviction_queue: ResMut<OctreeNodeEvictionQueue<T>>,
    global_visible_octree_nodes: Res<GlobalVisibleOctreeNodes<T>>,
    time: Res<Time<Real>>,
) {
    let elapsed = time.elapsed().as_millis();
    let eviction_queue = &mut octree_node_eviction_queue.eviction_queue;

    for key in &global_visible_octree_nodes.visible_octree_nodes {
        eviction_queue.push(key.clone(), Reverse(elapsed));
    }
}

/// This system remove nodes data to meet memory budget requirements, and update octree node total size
pub fn evict_octree_nodes<T: NodeData>(
    settings: Res<OctreeNodeEvictionSettings<T>>,
    mut octree_node_eviction_queue: ResMut<OctreeNodeEvictionQueue<T>>,
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
                .contains(key)
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
