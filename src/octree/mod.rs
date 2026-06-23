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
use bevy_render::{
    extract_component::{ExtractComponent, ExtractComponentPlugin},
    view::ExtractedView,
    RenderApp,
};
use node::NodeData;

use crate::octree::extract::render::{
    components::RenderVisibleOctreeNodes, resources::RenderOctreeIndex,
};

pub struct OctreeAssetPlugin<T, C>(PhantomData<fn() -> (T, C)>);

impl<T, C> Default for OctreeAssetPlugin<T, C> {
    fn default() -> Self {
        OctreeAssetPlugin(PhantomData)
    }
}
impl<T: NodeData, C: ExtractComponent> Plugin for OctreeAssetPlugin<T, C> {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<C>::default())
            .init_asset::<Octree<T>>()
            .init_resource::<OctreeTotalSize<T>>()
            .add_systems(First, reset_octree_nodes_tracking::<T>);

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .world_mut()
                .register_required_components::<ExtractedView, RenderVisibleOctreeNodes<T, C>>();

            render_app.init_resource::<RenderOctreeIndex<C>>();
        }
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
