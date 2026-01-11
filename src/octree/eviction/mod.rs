pub mod resources;

use crate::octree::asset::Octree;
use crate::octree::node::NodeData;
use crate::octree::visibility::resources::GlobalVisibleOctreeNodes;
use crate::octree::visibility::CheckOctreeNodesVisibility;
use bevy_app::{App, Plugin, PostUpdate};
use bevy_asset::{AssetId, Assets};
use bevy_ecs::prelude::*;
use bevy_time::{Real, Time};
use resources::OctreeNodeEvictionQueue;
use std::cmp::Reverse;
use std::marker::PhantomData;

pub struct OctreeEvictionPlugin<T, C>(PhantomData<fn() -> (T, C)>);

impl<T, C> OctreeEvictionPlugin<T, C> {}

impl<T, C> Default for OctreeEvictionPlugin<T, C> {
    fn default() -> Self {
        OctreeEvictionPlugin(PhantomData)
    }
}
impl<T, C> Plugin for OctreeEvictionPlugin<T, C>
where
    T: NodeData,
    C: Component,
    for<'a> &'a C: Into<AssetId<Octree<T>>>,
{
    fn build(&self, app: &mut App) {
        app.init_resource::<OctreeNodeEvictionQueue<T, C>>()
            .add_systems(
                PostUpdate,
                update_octree_node_eviction_queue::<T, C>.after(CheckOctreeNodesVisibility),
            );
    }
}

/// This system update the octree node eviction queue with latest informations
pub fn update_octree_node_eviction_queue<T: NodeData, C: Component>(
    mut octree_node_eviction_queue: ResMut<OctreeNodeEvictionQueue<T, C>>,
    global_visible_octree_nodes: Res<GlobalVisibleOctreeNodes<T, C>>,
    time: Res<Time<Real>>,
) {
    let elapsed = time.elapsed().as_millis();
    let eviction_queue = &mut octree_node_eviction_queue.eviction_queue;

    for key in &global_visible_octree_nodes.visible_octree_nodes {
        eviction_queue.push(key.clone(), Reverse(elapsed));
    }
}

/// This system mark octree nodes for eviction if needed
pub fn evict_octree_nodes<T: NodeData, C: Component>(
    mut octree_node_eviction_queue: ResMut<OctreeNodeEvictionQueue<T, C>>,
    global_visible_octree_nodes: Res<GlobalVisibleOctreeNodes<T, C>>,
    mut octrees: ResMut<Assets<Octree<T>>>,
    time: Res<Time<Real>>,
) {
    // TODO
}
