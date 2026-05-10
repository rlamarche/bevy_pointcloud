pub mod asset;

pub mod extract;
pub mod hierarchy;
pub mod loader;
pub mod node;
pub mod server;
pub mod storage;
pub mod visibility;

use std::marker::PhantomData;

use asset::Octree;
use bevy_app::{App, First, Plugin};
use bevy_asset::prelude::*;
use bevy_ecs::prelude::*;
use node::NodeData;

pub struct OctreeAssetPlugin<T>(PhantomData<fn() -> T>);

impl<T> Default for OctreeAssetPlugin<T> {
    fn default() -> Self {
        OctreeAssetPlugin(PhantomData)
    }
}
impl<T: NodeData> Plugin for OctreeAssetPlugin<T> {
    fn build(&self, app: &mut App) {
        app.init_asset::<Octree<T>>()
            .init_resource::<OctreeTotalSize<T>>()
            .add_systems(First, reset_octree_nodes_tracking::<T>);
    }
}

/// Update the total size of octree nodes and clear for next iteration
pub fn reset_octree_nodes_tracking<T: NodeData>(
    mut octree_total_size: ResMut<OctreeTotalSize<T>>,
    mut octrees: ResMut<Assets<Octree<T>>>,
) {
    let total_size = &mut octree_total_size.total_size;
    for (_, octree) in octrees.iter_mut() {
        for node_id in &octree.added_nodes_data {
            let Some(node) = octree.node(*node_id) else {
                continue;
            };
            let Some(data) = &node.data else {
                continue;
            };
            *total_size += data.size();
        }

        octree.clear_tracking();
    }
}

#[derive(Resource)]
pub struct OctreeTotalSize<T: NodeData> {
    pub(crate) total_size: usize,
    phantom: PhantomData<fn() -> T>,
}

impl<T: NodeData> Default for OctreeTotalSize<T> {
    fn default() -> Self {
        Self {
            total_size: 0,
            phantom: PhantomData,
        }
    }
}
