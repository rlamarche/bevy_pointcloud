pub mod resources;

use crate::octree::new_asset::asset::NewOctree;
use crate::octree::new_asset::hierarchy::{
    HierarchyNode, HierarchyNodeData, HierarchyNodeStatus, HierarchyOctreeNode,
};
use crate::octree::new_asset::server::OctreeServer;
use async_trait::async_trait;
use bevy_asset::prelude::*;
use bevy_ecs::error::BevyError;
use bevy_ecs::prelude::*;
use bevy_log::prelude::*;
use bevy_reflect::TypePath;
use resources::OctreeLoadTasks;
use std::fmt::Display;

#[async_trait]
pub trait OctreeLoader<H, T>: Send + Sync + Sized + TypePath
where
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
{
    type Error: Into<BevyError> + Send + Sync + Display;

    /// Instantiate a new hierarchy from a provided url
    async fn from_url(url: &str) -> Result<Self, Self::Error>;

    /// This method must load the initial octree hierarchy in a flat structure.
    /// The return value is a vector, the first item is the root,
    /// then all children are referenced in the parent with their indice in the vec.
    /// Every child should also reference its parent through its indice too.
    async fn load_initial_hierarchy(&self) -> Result<Vec<HierarchyNode<H>>, Self::Error>;

    /// This method must load the provided node sub hierarchy.
    /// The return format is the same as described in [`OctreeLoader::load_initial_hierarchy`].
    /// So, the provided node is expected to be the first in the returned vector.
    /// The provided node **must** be in [`HierarchyNodeStatus::Proxy`] state, or an error might be thrown.
    async fn load_hierarchy(
        &self,
        node: &HierarchyOctreeNode<H>,
    ) -> Result<Vec<HierarchyNode<H>>, Self::Error>;

    async fn load_node(
        &self,
        node: &HierarchyOctreeNode<H>,
    ) -> Result<T, Self::Error>;
}

pub fn process_octree_load_tasks<L, H, T>(
    mut load_tasks: ResMut<OctreeLoadTasks<H, T>>,
    mut octree_assets: ResMut<Assets<NewOctree<H, T>>>,
    mut server: ResMut<OctreeServer<L, H, T>>,
) where
    L: OctreeLoader<H, T>,
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
{
    const MAX_CONCURRENT_HIERARCHY: usize = 4;
    const MAX_CONCURRENT_NODES: usize = 8;

    // ========== Process hierarchy loads ==========
    process_hierarchy_loads(
        &mut load_tasks,
        &mut octree_assets,
        &mut server,
        MAX_CONCURRENT_HIERARCHY,
    );

    // ========== Process node loads ==========
    // process_node_data_loads(
    //     &mut load_tasks,
    //     &mut octree_assets,
    //     &thread_pool,
    //     MAX_CONCURRENT_NODES,
    // );
}

fn process_hierarchy_loads<L, H, T>(
    load_tasks: &mut OctreeLoadTasks<H, T>,
    octree_assets: &mut Assets<NewOctree<H, T>>,
    server: &mut ResMut<OctreeServer<L, H, T>>,
    max_concurrent: usize,
) where
    L: OctreeLoader<H, T>,
    H: HierarchyNodeData,
    T: Send + Sync + TypePath + 'static,
{
    while load_tasks.hierarchy_in_flight.len() < max_concurrent {
        // Pop highest weight task
        let Some(task) = load_tasks.hierarchy_heap.pop() else {
            break; // no more tasks
        };

        let key = (task.asset_id, task.node_id);

        // Check if this load is not already processed
        if load_tasks.hierarchy_in_flight.contains(&key) {
            continue;
        }

        let Some(octree) = octree_assets.get_mut(task.asset_id) else {
            warn!("Octree asset not found: {:?}", task.asset_id);
            continue;
        };

        let Some(node) = octree.hierarchy_node_mut(task.node_id) else {
            warn!("Node not found in octree: {:?}", task.node_id);
            continue;
        };

        // Check that we still need to load this node
        let should_load = matches!(node.status, HierarchyNodeStatus::Proxy);

        if !should_load {
            continue;
        }

        // Set loading status of the node
        node.status = HierarchyNodeStatus::Loading;

        // Spawn load sub hierarchy task
        if let Err(error) = server.load_sub_hierarchy(task.asset_id, octree, task.node_id) {
            warn!("An error occured loading node hierarchy: {:#} ", error);
            continue;
        }

        // Set in flight
        load_tasks.hierarchy_in_flight.insert(key);
    }
}
