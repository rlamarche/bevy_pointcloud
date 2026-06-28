use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    hash::{Hash, Hasher},
};

use bevy::{
    asset::{AssetId, Assets, InvalidGenerationError},
    ecs::{
        component::Component,
        hierarchy::ChildOf,
        resource::Resource,
        system::{Commands, Res, ResMut},
    },
    log::{debug, info, warn},
    platform::collections::HashSet,
};
use ordered_float::OrderedFloat;

use crate::{
    server::build_hierarchy_children, ChildChunkOf, HierarchyNodeStatus, InternalPointCloudEvent,
    NodeId, PointCloud, PointCloudChunk, PointCloudChunk3d, PointCloudChunkKey,
    PointCloudInstances, PointCloudNodeKey, PointCloudServer, PointCloudServerSettings,
    PointCloudTotalSize, PointCloudTracking,
};

#[derive(Resource, Default)]
pub struct PointCloudLoadTasks {
    pub hierarchy_heap: BinaryHeap<WeightedPointCloudLoadTask>,
    pub chunk_heap: BinaryHeap<WeightedPointCloudLoadTask>,
    pub hierarchy_in_flight: HashSet<PointCloudNodeKey>,
    pub chunk_in_flight: HashSet<PointCloudNodeKey>,
}

#[derive(Clone, Debug)]
pub enum LoadRequestType {
    Hierarchy,
    Chunk,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskId {
    LoadInitialHierarchy(AssetId<PointCloud>),
    LoadSubHierarchy(PointCloudNodeKey),
    LoadChunk(PointCloudNodeKey),
}

impl PointCloudLoadTasks {
    pub fn queue_load_request(
        &mut self,
        id: AssetId<PointCloud>,
        node_id: NodeId,
        weight: OrderedFloat<f32>,
        request_type: LoadRequestType,
    ) {
        let key = PointCloudNodeKey { id, node_id };

        match request_type {
            LoadRequestType::Hierarchy => {
                if !self.hierarchy_in_flight.contains(&key) {
                    self.hierarchy_heap.push(WeightedPointCloudLoadTask(
                        PointCloudLoadTask {
                            asset_id: id,
                            node_id,
                        },
                        weight,
                    ));
                }
            }
            LoadRequestType::Chunk => {
                if !self.chunk_in_flight.contains(&key) {
                    self.chunk_heap.push(WeightedPointCloudLoadTask(
                        PointCloudLoadTask {
                            asset_id: id,
                            node_id,
                        },
                        weight,
                    ));
                }
            }
        }
    }
}

#[derive(Debug, Component)]
pub struct PointCloudLoadTask {
    pub asset_id: AssetId<PointCloud>,
    pub node_id: NodeId,
}

impl Hash for PointCloudLoadTask {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.asset_id.hash(state);
        self.node_id.hash(state);
    }
}

impl PartialEq<Self> for PointCloudLoadTask {
    fn eq(&self, other: &Self) -> bool {
        self.asset_id == other.asset_id && self.node_id == other.node_id
    }
}

impl Eq for PointCloudLoadTask {}

pub struct WeightedPointCloudLoadTask(pub PointCloudLoadTask, pub OrderedFloat<f32>);

impl PartialEq<Self> for WeightedPointCloudLoadTask {
    fn eq(&self, other: &Self) -> bool {
        self.1.eq(&other.1)
    }
}

impl Eq for WeightedPointCloudLoadTask {}

impl PartialOrd<Self> for WeightedPointCloudLoadTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WeightedPointCloudLoadTask {
    fn cmp(&self, other: &Self) -> Ordering {
        self.1.cmp(&other.1)
    }
}

/// A system that manages internal [`PointCloudServer`] events, such as finalizing asset loads.
pub fn handle_internal_point_cloud_events(
    server: Res<PointCloudServer>,
    mut point_clouds: ResMut<Assets<PointCloud>>,
    mut chunks: ResMut<Assets<PointCloudChunk>>,
    mut load_tasks: ResMut<PointCloudLoadTasks>,
    mut point_cloud_tracking: ResMut<PointCloudTracking>,
    point_cloud_instances: Res<PointCloudInstances>,
    mut commands: Commands,
) {
    // clone `server.data` because we need to borrow server as mutable in the loop
    for event in server.data.clone().event_receiver.try_iter() {
        match event {
            InternalPointCloudEvent::Loaded {
                id,
                loaded_asset,
                loader,
            } => {
                let aabb = loaded_asset.aabb;

                // store the asset in the assets resource
                match point_clouds.insert(id, loaded_asset) {
                    Ok(_) => {}
                    Err(err) => match err {
                        InvalidGenerationError::Occupied { .. } => {
                            panic!("Invalid asset id encountered");
                        }
                        InvalidGenerationError::Removed { .. } => {
                            // The asset has been removed, continue with no error
                            continue;
                        }
                    },
                }

                // store the loader in the server
                {
                    let mut loaders = server.write_loaders();
                    loaders.insert(id, loader);
                }

                if let Some(aabb) = aabb
                    && let Some(point_cloud_entities) = point_cloud_instances.get(&id)
                {
                    for (&point_cloud_entity, _) in point_cloud_entities.iter() {
                        commands.entity(point_cloud_entity).insert(aabb);
                    }
                }

                info!("Loaded PointCloud {:?} with initial hierarchy", id);
            }
            InternalPointCloudEvent::SubHierarchyLoaded {
                id,
                node_id,
                mut hierarchy_nodes,
            } => {
                let key = PointCloudNodeKey { id, node_id };

                // update in flight hashset
                load_tasks.hierarchy_in_flight.remove(&key);

                let Some(mut point_cloud) = point_clouds.get_mut(id) else {
                    debug!(
                        "No asset found for {:?}, unable to append loaded hierarchy nodes.",
                        id
                    );
                    continue;
                };

                if hierarchy_nodes.is_empty() {
                    warn!(
                        "Loaded empty hierarchy for {:?}/{:?}, skipping update.",
                        id, node_id
                    );
                    continue;
                }

                let (children, roots) = build_hierarchy_children(&hierarchy_nodes);

                let Some(root_idx) = roots.first().copied() else {
                    warn!(
                        "Loaded hierarchy for {:?}/{:?} is missing a root node.",
                        id, node_id
                    );
                    continue;
                };

                if roots.len() > 1 {
                    warn!(
                        "Loaded hierarchy for {:?}/{:?} contains {} root nodes; using the first one",
                        id,
                        node_id,
                        roots.len()
                    );
                }

                let mut inserted_nodes: Vec<Option<NodeId>> = vec![None; hierarchy_nodes.len()];

                let hierarchy_node = std::mem::take(&mut hierarchy_nodes[root_idx]);
                match point_cloud.hierarchy.get_node_mut(node_id) {
                    Some(node) => {
                        node.status = hierarchy_node.status;
                        node.data = hierarchy_node.data;
                        if let Some(aabb) = hierarchy_node.aabb {
                            node.aabb = Some(aabb);
                        }
                        inserted_nodes[root_idx] = Some(node_id);
                    }
                    None => {
                        warn!(
                            "Hierarchy node {:?} not found for asset {:?} when updating hierarchy.",
                            node_id, id
                        );
                        continue;
                    }
                }

                let mut stack: Vec<(usize, NodeId)> = children[root_idx]
                    .iter()
                    .rev()
                    .map(|&child_idx| (child_idx, node_id))
                    .collect();

                while let Some((idx, parent)) = stack.pop() {
                    if inserted_nodes[idx].is_some() {
                        continue;
                    }

                    let hierarchy_node = std::mem::take(&mut hierarchy_nodes[idx]);

                    let new_id = match point_cloud.hierarchy.insert_hierarchy_node(
                        Some(parent),
                        hierarchy_node.child_index,
                        hierarchy_node.status,
                        hierarchy_node.point_count,
                        hierarchy_node.data,
                        hierarchy_node.aabb,
                        None,
                    ) {
                        Ok(node_id) => node_id,
                        Err(error) => {
                            warn!(
                                "Unable to insert new hierarchy node for asset {:?}: {:#}",
                                id, error
                            );
                            continue;
                        }
                    };

                    inserted_nodes[idx] = Some(new_id);

                    for &child_idx in children[idx].iter().rev() {
                        stack.push((child_idx, new_id));
                    }
                }
            }
            InternalPointCloudEvent::SubHierarchyLoadFailed { id, node_id, error } => {
                let key = PointCloudNodeKey { id, node_id };

                // update in flight hashset
                load_tasks.hierarchy_in_flight.remove(&key);

                warn!("An error occured loading sub hierarchy: {:#}", error);

                if let Some(mut point_cloud) = point_clouds.get_mut(id)
                    && let Some(hierarchy_node) = point_cloud.hierarchy.get_node_mut(node_id)
                {
                    hierarchy_node.status = HierarchyNodeStatus::Proxy;
                } else {
                    debug!(
                        "No asset found for {:?}, unable to append loaded hierarchy nodes.",
                        id
                    );
                }
            }
            InternalPointCloudEvent::ChunkLoaded { id, node_id, mesh } => {
                info!("Chunk loaded: {:?}/{:?}", id, node_id);
                let key = PointCloudNodeKey { id, node_id };

                // update in flight hashset
                load_tasks.chunk_in_flight.remove(&key);

                let Some(mut point_cloud) = point_clouds.get_mut(id) else {
                    warn!("No asset found for {:?}, unable to store node data.", id);
                    continue;
                };

                let Some(node) = point_cloud.hierarchy.get_node_mut(node_id) else {
                    warn!(
                        "Hierarchy node {:?} not found for asset {:?} when storing chunk.",
                        node_id, id
                    );
                    continue;
                };

                let vertex_buffer_size = mesh.get_vertex_buffer_size();

                let chunk_handle = chunks.add(PointCloudChunk {
                    mesh_handle: None,
                    aabb: node.aabb,
                    vertex_buffer_size,
                });

                let chunk_key = PointCloudChunkKey {
                    id,
                    chunk_id: chunk_handle.id(),
                };

                point_cloud_tracking.loaded_chunks.push((chunk_key, mesh));

                node.chunk = Some(chunk_handle.clone());

                // get again the node immutably
                let node = point_cloud.hierarchy.get_node(node_id).unwrap(); // was valid just above

                if let Some(point_cloud_entities) = point_cloud_instances.get(&id) {
                    for (&point_cloud_entity, chunks) in point_cloud_entities.iter() {
                        if let Some(parent_node_id) = node.parent_id
                            && let Some(parent_hierarchy_node) =
                                point_cloud.hierarchy.get_node(parent_node_id)
                            && let Some(parent_handle_id) = &parent_hierarchy_node.chunk
                        {
                            let Some(&parent_chunk_entity) = chunks.get(&parent_handle_id.id())
                            else {
                                warn!("Missing parent entity chunk for node id {:?}", node_id);
                                continue;
                            };
                            commands.spawn((
                                PointCloudChunk3d(chunk_handle.clone()),
                                ChildOf(parent_chunk_entity),
                                ChildChunkOf(point_cloud_entity),
                            ));
                        } else {
                            // this is the root chunk
                            commands.spawn((
                                PointCloudChunk3d(chunk_handle.clone()),
                                ChildChunkOf(point_cloud_entity),
                            ));
                        }
                    }
                } else {
                    warn!("problem");
                }
            }
            InternalPointCloudEvent::ChunkLoadFailed { id, node_id, error } => {
                let key = PointCloudNodeKey { id, node_id };

                // update in flight hashset
                load_tasks.chunk_in_flight.remove(&key);

                warn!("An error occured loading chunk: {:#}", error);

                // let Some(point_cloud) = point_clouds.get_mut(id) else {
                //     debug!(
                //         "No asset found for {:?}, unable to append loaded hierarchy nodes.",
                //         id
                //     );
                //     continue;
                // };

                // let _ = point_cloud.unset_node_data_loading(node_id);
            }
        }
    }

    #[cfg(not(any(target_arch = "wasm32", not(feature = "multi_threaded"))))]
    server
        .write_infos()
        .pending_tasks
        .retain(|_, load_task| !load_task.is_finished());
}

pub fn process_point_cloud_load_tasks(
    mut load_tasks: ResMut<PointCloudLoadTasks>,
    mut point_cloud_assets: ResMut<Assets<PointCloud>>,
    mut server: ResMut<PointCloudServer>,
    point_cloud_total_size: Res<PointCloudTotalSize>,
    settings: Res<PointCloudServerSettings>,
) {
    // ========== Process hierarchy loads ==========
    process_hierarchy_loads(
        &mut load_tasks,
        &mut point_cloud_assets,
        &mut server,
        settings.max_concurrent_hierarchy_load_task,
    );

    // ========== Process node loads ==========
    process_chunk_loads(
        &mut load_tasks,
        &mut point_cloud_assets,
        &mut server,
        settings.max_concurrent_chunks_load_task,
        &point_cloud_total_size,
        &settings,
    );
}

fn process_hierarchy_loads(
    load_tasks: &mut PointCloudLoadTasks,
    point_cloud_assets: &mut Assets<PointCloud>,
    server: &mut ResMut<PointCloudServer>,
    max_concurrent: usize,
) {
    while load_tasks.hierarchy_in_flight.len() < max_concurrent {
        // Pop highest weight task
        let Some(WeightedPointCloudLoadTask(task, ..)) = load_tasks.hierarchy_heap.pop() else {
            break; // no more tasks
        };

        let key = PointCloudNodeKey {
            id: task.asset_id,
            node_id: task.node_id,
        };

        // Check if this load is not already in processing
        if load_tasks.hierarchy_in_flight.contains(&key) {
            continue;
        }

        let Some(mut point_cloud) = point_cloud_assets.get_mut(task.asset_id) else {
            warn!(
                "PointCloud asset not found when loading hierarchy: {:?}",
                task.asset_id
            );
            continue;
        };

        let Some(node) = point_cloud.hierarchy.get_node(task.node_id) else {
            warn!(
                "Node not found in point_cloud when loading hierarchy: {:?}",
                task.node_id
            );
            continue;
        };

        // Check that we still need to load this node
        let should_load = matches!(node.status, HierarchyNodeStatus::Proxy);

        if !should_load {
            continue;
        }

        // Spawn load sub hierarchy task
        if let Err(error) = server.load_sub_hierarchy(task.asset_id, &mut point_cloud, task.node_id)
        {
            warn!("An error occured when loading node hierarchy: {:#} ", error);
            continue;
        }

        // Set in flight
        load_tasks.hierarchy_in_flight.insert(key);
    }
}

fn process_chunk_loads(
    load_tasks: &mut PointCloudLoadTasks,
    point_cloud_assets: &mut Assets<PointCloud>,
    server: &mut ResMut<PointCloudServer>,
    max_concurrent: usize,
    point_cloud_total_size: &PointCloudTotalSize,
    // TODO fallback if plugin not enabled ?
    settings: &PointCloudServerSettings,
) {
    // do not try to load nodes if max memory is reached
    if point_cloud_total_size.total_size > settings.max_size {
        return;
    }

    while load_tasks.chunk_in_flight.len() < max_concurrent {
        // Pop highest weight task
        let Some(WeightedPointCloudLoadTask(task, ..)) = load_tasks.chunk_heap.pop() else {
            break; // no more tasks
        };

        let key = PointCloudNodeKey {
            id: task.asset_id,
            node_id: task.node_id,
        };

        // Check if this load is not already processed
        if load_tasks.chunk_in_flight.contains(&key) {
            continue;
        }

        let Some(point_cloud) = point_cloud_assets.get_mut(task.asset_id) else {
            debug!(
                "PointCloud asset not found when loading chunk: {:?}",
                task.asset_id
            );
            continue;
        };

        let Some(node) = point_cloud.hierarchy.get_node(task.node_id) else {
            warn!(
                "Node not found in point_cloud when loading chunk: {:?}",
                task.node_id
            );
            continue;
        };

        // Check that we still need to load this chunk
        let should_load = node.chunk.is_none();

        if !should_load {
            continue;
        }

        // Spawn load sub hierarchy task
        if let Err(error) = server.load_chunk(task.asset_id, &point_cloud, task.node_id) {
            warn!("An error occured when loading chunk data: {:#} ", error);
            continue;
        }

        // Set in flight
        load_tasks.chunk_in_flight.insert(key);
    }
}
