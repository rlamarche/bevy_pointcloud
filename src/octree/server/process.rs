use super::{
    super::{
        asset::Octree,
        hierarchy::HierarchyNodeStatus,
        node::{NodeData, NodeStatus},
    },
    resources::{OctreeLoadTasks, WeightedOctreeNodeLoadTask},
    OctreeServer,
};
use crate::octree::{server::resources::OctreeServerSettings, OctreeTotalSize};
use bevy_asset::prelude::*;
use bevy_ecs::prelude::*;
use bevy_log::prelude::*;

pub fn process_octree_load_tasks<T>(
    mut load_tasks: ResMut<OctreeLoadTasks<T>>,
    mut octree_assets: ResMut<Assets<Octree<T>>>,
    mut server: ResMut<OctreeServer<T>>,
    octree_total_size: Res<OctreeTotalSize<T>>,
    settings: Res<OctreeServerSettings<T>>,
) where
    T: NodeData,
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
    process_node_data_loads(
        &mut load_tasks,
        &mut octree_assets,
        &mut server,
        MAX_CONCURRENT_NODES,
        &octree_total_size,
        &settings,
    );
}

fn process_hierarchy_loads<T>(
    load_tasks: &mut OctreeLoadTasks<T>,
    octree_assets: &mut Assets<Octree<T>>,
    server: &mut ResMut<OctreeServer<T>>,
    max_concurrent: usize,
) where
    T: NodeData,
{
    while load_tasks.hierarchy_in_flight.len() < max_concurrent {
        // Pop highest weight task
        let Some(WeightedOctreeNodeLoadTask(task, ..)) = load_tasks.hierarchy_heap.pop() else {
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

        let Some(node) = octree.hierarchy_node(task.node_id) else {
            warn!("Node not found in octree: {:?}", task.node_id);
            continue;
        };

        // Check that we still need to load this node
        let should_load = matches!(node.status, HierarchyNodeStatus::Proxy);

        if !should_load {
            continue;
        }

        // Spawn load sub hierarchy task
        if let Err(error) = server.load_sub_hierarchy(task.asset_id, octree, task.node_id) {
            warn!("An error occured loading node hierarchy: {:#} ", error);
            continue;
        }

        // Set in flight
        load_tasks.hierarchy_in_flight.insert(key);
    }
}

fn process_node_data_loads<T>(
    load_tasks: &mut OctreeLoadTasks<T>,
    octree_assets: &mut Assets<Octree<T>>,
    server: &mut ResMut<OctreeServer<T>>,
    max_concurrent: usize,
    octree_total_size: &OctreeTotalSize<T>,
    // TODO fallback if plugin not enabled ?
    settings: &OctreeServerSettings<T>,
) where
    T: NodeData,
{
    // do not try to load nodes if max memory is reached
    if octree_total_size.total_size > settings.max_size {
        return;
    }

    while load_tasks.node_in_flight.len() < max_concurrent {
        // Pop highest weight task
        let Some(WeightedOctreeNodeLoadTask(task, ..)) = load_tasks.node_heap.pop() else {
            break; // no more tasks
        };

        let key = (task.asset_id, task.node_id);

        // Check if this load is not already processed
        if load_tasks.node_in_flight.contains(&key) {
            continue;
        }

        let Some(octree) = octree_assets.get_mut(task.asset_id) else {
            warn!("Octree asset not found: {:?}", task.asset_id);
            continue;
        };

        let Some(node) = octree.node(task.node_id) else {
            warn!("Node not found in octree: {:?}", task.node_id);
            continue;
        };

        // Check that we still need to load this node
        let should_load = matches!(node.status, NodeStatus::HierarchyOnly);

        if !should_load {
            continue;
        }

        // Spawn load sub hierarchy task
        if let Err(error) = server.load_node_data(task.asset_id, octree, task.node_id) {
            warn!("An error occured loading node data: {:#} ", error);
            continue;
        }

        // Set in flight
        load_tasks.node_in_flight.insert(key);
    }
}
