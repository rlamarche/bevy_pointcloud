use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    hash::{Hash, Hasher},
};

use bevy::{
    asset::{AssetId, Assets, InvalidGenerationError},
    ecs::{
        component::Component,
        error::BevyError,
        hierarchy::ChildOf,
        resource::Resource,
        system::{Commands, Res, ResMut},
    },
    log::{debug, info, warn},
    platform::collections::{HashMap, HashSet},
};
use ordered_float::OrderedFloat;

use crate::{
    ChildChunkOf, ErasedOctreeHierarchy, InsertNodeParams, InternalPointCloudEvent, NodeId,
    OctreeTopology, PointCloud, PointCloudChunk, PointCloudChunk3d, PointCloudChunkKey,
    PointCloudInstances, PointCloudNodeKey, PointCloudNodeStatus, PointCloudServer,
    PointCloudServerSettings, PointCloudTotalSize, PointCloudTracking,
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
                // TODO: cleanup loaders
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
                octree_hierarchy,
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

                let Some(octree_topology) = point_cloud.topology.as_octree_mut() else {
                    warn!(
                        "Loading sub hierarchy for a point cloud whose topology is not an octree.",
                    );
                    continue;
                };

                // remove the previous proxy node
                // let Some(node) = octree_topology.remove_node(node_id) else {
                //     warn!(
                //         "Hierarchy node {:?} not found for asset {:?} when updating hierarchy.",
                //         node_id, id
                //     );
                //     continue;
                // };

                if let Err(e) = append_hierarchy(octree_topology, octree_hierarchy, Some(node_id)) {
                    warn!(
                        "An error occured when appending hierarchy to point cloud octree {:?}: {:#}",
                        id,
                        e,
                    );
                }
            }
            InternalPointCloudEvent::SubHierarchyLoadFailed { id, node_id, error } => {
                let key = PointCloudNodeKey { id, node_id };

                // update in flight hashset
                load_tasks.hierarchy_in_flight.remove(&key);

                warn!("An error occured loading sub hierarchy: {:#}", error);

                if let Some(mut point_cloud) = point_clouds.get_mut(id)
                    && let Some(octree) = point_cloud.topology.as_octree_mut()
                    && let Some(hierarchy_node) = octree.get_node_mut(node_id)
                {
                    hierarchy_node.status = PointCloudNodeStatus::Proxy;
                } else {
                    debug!(
                        "No asset found for {:?}, unable to append loaded hierarchy nodes.",
                        id
                    );
                }
            }
            InternalPointCloudEvent::ChunkLoaded {
                id,
                node_id,
                result,
            } => {
                let key = PointCloudNodeKey { id, node_id };

                // update in flight hashset
                load_tasks.chunk_in_flight.remove(&key);

                let Some(mut point_cloud) = point_clouds.get_mut(id) else {
                    warn!("No asset found for {:?}, unable to store node data.", id);
                    continue;
                };

                let topology = (&point_cloud.topology).into();
                let Some(octree) = point_cloud.topology.as_octree_mut() else {
                    warn!(
                        "Point cloud {:?} is not an octree, unable to store node data.",
                        id
                    );
                    continue;
                };

                let Some(node) = octree.get_node_mut(node_id) else {
                    warn!(
                        "Hierarchy node {:?} not found for asset {:?} when storing chunk.",
                        node_id, id
                    );
                    continue;
                };

                // If there is no mesh, it means that this node is empty.
                // Sets its point count to 0 and continue.
                let Some(mesh) = result.mesh else {
                    node.point_count = 0;
                    continue;
                };

                let vertex_buffer_size = mesh.get_vertex_buffer_size();

                let chunk_handle = chunks.add(PointCloudChunk {
                    topology,
                    depth: node.depth,
                    offset: result.offset,
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
                node.offset = result.offset;
                node.point_count = result.final_point_count;

                // get again the node immutably
                let node = octree.get_node(node_id).unwrap(); // was valid just above

                if let Some(point_cloud_entities) = point_cloud_instances.get(&id) {
                    for (&point_cloud_entity, chunks) in point_cloud_entities.iter() {
                        if let Some(parent_node_id) = node.parent_id
                            && let Some(parent_hierarchy_node) = octree.get_node(parent_node_id)
                            && let Some(parent_handle_id) = &parent_hierarchy_node.chunk
                        {
                            let Some(&parent_chunk_entity) = chunks.get(&parent_handle_id.id())
                            else {
                                warn!("Missing parent entity chunk for node id {:?}", node_id);
                                continue;
                            };
                            let mut entity = commands.spawn((
                                PointCloudChunk3d(chunk_handle.clone()),
                                // the parent is the point cloud entity (direct link to root)
                                ChildOf(point_cloud_entity),
                                // but also a link to the parent chunk
                                ChildChunkOf(parent_chunk_entity),
                            ));
                            if let Some(aabb) = node.aabb {
                                entity.insert(aabb);
                            }
                        } else {
                            // this is the root chunk, add it to the root component (the one with
                            // the [`PointCloud3d`] component)
                            commands
                                .entity(point_cloud_entity)
                                .insert(PointCloudChunk3d(chunk_handle.clone()));
                        }
                    }
                }
            }
            InternalPointCloudEvent::ChunkLoadFailed { id, node_id, error } => {
                let key = PointCloudNodeKey { id, node_id };

                // update in flight hashset
                load_tasks.chunk_in_flight.remove(&key);

                warn!("An error occured loading chunk: {:#}", error);
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

        let Some(octree) = point_cloud.topology.as_octree_mut() else {
            warn!(
                "PointCloud asset is not an octree when loading hierarchy: {:?}",
                task.asset_id
            );
            continue;
        };

        let Some(node) = octree.get_node(task.node_id) else {
            warn!(
                "Node not found in point_cloud when loading hierarchy: {:?}",
                task.node_id
            );
            continue;
        };

        // Check that we still need to load this node
        let should_load = matches!(node.status, PointCloudNodeStatus::Proxy);

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

        let Some(point_cloud) = point_cloud_assets.get(task.asset_id) else {
            debug!(
                "PointCloud asset not found when loading chunk: {:?}",
                task.asset_id
            );
            continue;
        };

        let Some(octree) = point_cloud.topology.as_octree() else {
            debug!(
                "PointCloud asset is not an octree when loading chunk: {:?}",
                task.asset_id
            );
            continue;
        };

        let Some(node) = octree.get_node(task.node_id) else {
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
        if let Err(error) = server.load_chunk(task.asset_id, point_cloud, task.node_id) {
            warn!("An error occured when loading chunk data: {:#} ", error);
            continue;
        }

        // Set in flight
        load_tasks.chunk_in_flight.insert(key);
    }
}

pub fn append_hierarchy(
    octree_topology: &mut OctreeTopology,
    octree_hierarchy: ErasedOctreeHierarchy,
    root_id: Option<NodeId>,
) -> Result<(), BevyError> {
    let Some(root) = octree_hierarchy.get_root() else {
        return Err(BevyError::from(
            "Loaded point cloud hierarchy is empty or missing a root node",
        ));
    };

    let mut stack = vec![(root.id, None)];
    let mut node_map = HashMap::new();
    while let Some((builder_node_id, parent_id)) = stack.pop() {
        let Some(node) = octree_hierarchy.get_node(builder_node_id) else {
            // this shouldn't happen
            warn!("A node is missing in the octree hierarchy, this should'nt happen.d");
            continue;
        };

        let node_id = match parent_id {
            Some(parent_id) => octree_topology.insert_child(
                parent_id,
                node.child_index,
                InsertNodeParams {
                    status: node.status,
                    point_count: node.point_count,
                    aabb: node.aabb,
                },
                node.data.clone(),
            )?,
            None => {
                if let Some(root_id) = root_id {
                    if let Some(root) = octree_topology.get_node_mut(root_id) {
                        root.status = node.status;
                        root.point_count = node.point_count;
                        root.data = node.data.clone();
                        root.aabb = node.aabb;
                    }
                    root_id
                } else {
                    octree_topology.insert_root(
                        InsertNodeParams {
                            status: node.status,
                            point_count: node.point_count,
                            aabb: node.aabb,
                        },
                        node.data.clone(),
                    )?
                }
            }
        };

        node_map.insert(builder_node_id, node_id);

        for child_index in node.children_mask.iter_one_bits() {
            stack.push((node.children[child_index as usize], Some(node_id)));
        }
    }
    Ok(())
}
