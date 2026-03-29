pub mod process;
pub mod resources;
pub mod systems;
mod task;

use super::asset::Octree;
use super::hierarchy::{HierarchyNode, HierarchyOctreeNode};
use super::loader::{ErasedOctreeLoader, OctreeLoader};
use super::node::NodeData;
use super::visibility::CheckOctreeNodesVisibility;
use crate::octree::server::systems::{
    evict_octree_nodes, update_octree_server_node_eviction_queue,
};
use crate::octree::storage::NodeId;
use bevy_app::prelude::*;
use bevy_asset::{AssetHandleProvider, AssetId, Assets, Handle};
use bevy_ecs::prelude::*;
use bevy_log::prelude::*;
use bevy_platform::collections::HashMap;
use bevy_tasks::IoTaskPool;
use crossbeam::channel::{Receiver, Sender};
use process::process_octree_load_tasks;
use resources::OctreeLoadTasks;
use resources::{OctreeServerEvictionQueue, OctreeServerSettings};
use std::marker::PhantomData;
use std::sync::Arc;
use task::spawn_async_task;
use thiserror::Error;

pub struct OctreeServerPlugin<T> {
    pub max_size: usize,
    _phantom: PhantomData<fn() -> T>,
}

impl<T> Default for OctreeServerPlugin<T> {
    fn default() -> Self {
        OctreeServerPlugin {
            max_size: 1024 * 1024 * 1024, // 1024 mb
            _phantom: PhantomData,
        }
    }
}

impl<T> OctreeServerPlugin<T> {
    /// Construct with specific max memory size for CPU
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            max_size,
            _phantom: PhantomData,
        }
    }
}

impl<T> Plugin for OctreeServerPlugin<T>
where
    T: NodeData,
{
    fn build(&self, app: &mut App) {
        app.insert_resource(OctreeServerSettings::<T> {
            max_size: self.max_size,
            _phantom: PhantomData,
        })
        .init_resource::<OctreeServer<T>>()
        .init_resource::<OctreeServerEvictionQueue<T>>()
        .add_systems(
            PostUpdate,
            (
                handle_internal_octree_events::<T>,
                process_octree_load_tasks::<T>.after(CheckOctreeNodesVisibility),
                update_octree_server_node_eviction_queue::<T>.after(CheckOctreeNodesVisibility),
                evict_octree_nodes::<T>.after(update_octree_server_node_eviction_queue::<T>),
            ),
        );
    }
}

#[derive(Resource)]
pub struct OctreeServer<T>
where
    T: NodeData,
{
    pub(crate) data: Arc<OctreeServerData<T>>,
    pub(crate) loaders: HashMap<AssetId<Octree<T>>, Arc<dyn ErasedOctreeLoader<T>>>,
}

pub struct OctreeServerData<T>
where
    T: NodeData,
{
    pub(crate) handle_provider: AssetHandleProvider,
    pub(crate) octree_event_sender: Sender<InternalOctreeEvent<T>>,
    pub(crate) octree_event_receiver: Receiver<InternalOctreeEvent<T>>,
}

impl<T> OctreeServerData<T>
where
    T: NodeData,
{
    async fn load_internal<L: OctreeLoader<T>>(
        &self,
        url: &str,
        handle: Handle<Octree<T>>,
    ) -> Result<(), BevyError> {
        let loader = L::from_url(url).await.map_err(|error| error.into())?;
        let asset_id = handle.id();

        let mut octree = Octree::<T>::new();

        let initial_hierarchy = loader
            .load_initial_hierarchy()
            .await
            .map_err(|error| error.into())?
            .into_iter()
            .map(Into::into)
            .collect::<Vec<HierarchyNode>>();

        let (children, roots) = build_hierarchy_children(&initial_hierarchy);

        let Some(&root_idx) = roots.first() else {
            return Err(BevyError::from(
                "Loaded octree hierarchy is empty or missing a root node",
            ));
        };

        if roots.len() > 1 {
            warn!(
                "Loaded octree hierarchy contains {} root nodes; using the first one",
                roots.len()
            );
        }

        let mut inserted_nodes: Vec<Option<NodeId>> = vec![None; initial_hierarchy.len()];
        let mut stack = vec![(root_idx, None)];

        while let Some((idx, parent_id)) = stack.pop() {
            if inserted_nodes[idx].is_some() {
                continue;
            }

            let node_id =
                octree.insert_hierarchy_node(parent_id, initial_hierarchy[idx].clone())?;
            inserted_nodes[idx] = Some(node_id);

            for &child_idx in children[idx].iter().rev() {
                stack.push((child_idx, Some(node_id)));
            }
        }

        self.octree_event_sender
            .send(InternalOctreeEvent::Loaded {
                id: asset_id,
                loaded_asset: octree,
                loader: Arc::new(loader),
            })
            .expect("Failed to send internal octree event");

        Ok(())
    }

    async fn load_sub_hierarchy_internal(
        &self,
        id: AssetId<Octree<T>>,
        loader: Arc<dyn ErasedOctreeLoader<T>>,
        hierarchy_node: &HierarchyOctreeNode,
    ) -> Result<(), BevyError> {
        match loader.load_hierarchy(hierarchy_node).await {
            Ok(loaded_hierarchy_nodes) => {
                let hierarchy_nodes = loaded_hierarchy_nodes.into_iter().map(Into::into).collect();

                self.octree_event_sender
                    .send(InternalOctreeEvent::SubHierarchyLoaded {
                        id,
                        node_id: hierarchy_node.id,
                        hierarchy_nodes,
                    })
                    .expect("Failed to send internal octree event");
            }
            Err(err) => {
                error!("Error loading sub hierarchy: {}", err);
                self.octree_event_sender
                    .send(InternalOctreeEvent::SubHierarchyLoadFailed {
                        id,
                        node_id: hierarchy_node.id,
                        error: err.to_string(),
                    })
                    .expect("Failed to send internal octree event");
            }
        };

        Ok(())
    }

    async fn load_node_data_internal(
        &self,
        id: AssetId<Octree<T>>,
        loader: Arc<dyn ErasedOctreeLoader<T>>,
        hierarchy_node: &HierarchyOctreeNode,
    ) -> Result<(), BevyError> {
        match loader.load_node_data(hierarchy_node).await {
            Ok(node_data) => self
                .octree_event_sender
                .send(InternalOctreeEvent::NodeDataLoaded {
                    id,
                    node_id: hierarchy_node.id,
                    node_data,
                })
                .expect("Failed to send internal octree event"),
            Err(err) => {
                error!("Error loading node data: {}", err);
                self.octree_event_sender
                    .send(InternalOctreeEvent::NodeDataLoadFailed {
                        id,
                        node_id: hierarchy_node.id,
                        error: err.to_string(),
                    })
                    .expect("Failed to send internal octree event");
            }
        }

        Ok(())
    }
}

/// Internal events for asset load results
pub(crate) enum InternalOctreeEvent<T>
where
    T: NodeData,
{
    Loaded {
        id: AssetId<Octree<T>>,
        loaded_asset: Octree<T>,
        loader: Arc<dyn ErasedOctreeLoader<T>>,
    },
    SubHierarchyLoaded {
        id: AssetId<Octree<T>>,
        node_id: NodeId,
        hierarchy_nodes: Vec<HierarchyNode>,
    },
    SubHierarchyLoadFailed {
        id: AssetId<Octree<T>>,
        node_id: NodeId,
        #[allow(unused)]
        error: String,
    },
    NodeDataLoaded {
        id: AssetId<Octree<T>>,
        node_id: NodeId,
        node_data: T,
    },
    NodeDataLoadFailed {
        id: AssetId<Octree<T>>,
        node_id: NodeId,
        #[allow(unused)]
        error: String,
    },
}

/// Build a child adjacency list and collect root indices for hierarchy vectors.
fn build_hierarchy_children(nodes: &[HierarchyNode]) -> (Vec<Vec<usize>>, Vec<usize>) {
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    let mut roots = Vec::new();

    for (idx, node) in nodes.iter().enumerate() {
        if let Some(parent) = node.parent_id {
            if parent < nodes.len() {
                children[parent].push(idx);
            } else {
                warn!(
                    "Hierarchy node {} references parent {} but only {} nodes exist",
                    idx,
                    parent,
                    nodes.len()
                );
            }
        } else {
            roots.push(idx);
        }
    }

    (children, roots)
}

impl<T> FromWorld for OctreeServer<T>
where
    T: NodeData,
{
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<Assets<Octree<T>>>();
        let handle_provider = asset_server.get_handle_provider();

        let (octree_event_sender, octree_event_receiver) = crossbeam::channel::unbounded();

        Self {
            data: Arc::new(OctreeServerData {
                handle_provider,
                octree_event_sender,
                octree_event_receiver,
            }),
            loaders: HashMap::new(),
        }
    }
}

impl<T> OctreeServer<T>
where
    T: NodeData,
{
    /// Load an octree lazily (octree content will be loaded on the fly when needed)
    pub fn load_octree<L: OctreeLoader<T>>(&self, url: &str) -> Handle<Octree<T>> {
        let url = url.to_string();
        let handle = self.data.handle_provider.reserve_handle().typed();
        let owned_handle = handle.clone();

        let data = self.data.clone();
        #[allow(unused)]
        let task = IoTaskPool::get().spawn(async move {
            if let Err(err) = data.load_internal::<L>(&url, owned_handle).await {
                error!("{}", err);
            }
        });

        #[cfg(any(target_arch = "wasm32", not(feature = "multi_threaded")))]
        task.detach();

        handle
    }

    pub fn load_sub_hierarchy(
        &mut self,
        asset_id: AssetId<Octree<T>>,
        asset: &mut Octree<T>,
        node_id: NodeId,
    ) -> Result<(), OctreeServerError> {
        asset
            .set_node_hierarchy_loading(node_id)
            .map_err(|_err| OctreeServerError::HierarchyNodeNotFound)?;

        let hierarchy_octree_node = asset
            .hierarchy_node(node_id)
            .ok_or(OctreeServerError::HierarchyNodeNotFound)?
            .clone();

        let Some(loader) = self.loaders.get(&asset_id).cloned() else {
            return Err(OctreeServerError::LoaderNotFound);
        };
        let data = self.data.clone();

        #[allow(unused)]
        let task = spawn_async_task(async move {
            if let Err(err) = data
                .load_sub_hierarchy_internal(asset_id, loader, &hierarchy_octree_node)
                .await
            {
                error!("{}", err);
            }
        });

        #[cfg(any(
            all(target_arch = "wasm32", not(feature = "wasm_worker")),
            not(feature = "multi_threaded")
        ))]
        task.detach();

        Ok(())
    }

    pub fn load_node_data(
        &mut self,
        asset_id: AssetId<Octree<T>>,
        asset: &mut Octree<T>,
        node_id: NodeId,
    ) -> Result<(), OctreeServerError> {
        asset
            .set_node_data_loading(node_id)
            .map_err(|_err| OctreeServerError::HierarchyNodeNotFound)?;

        let hierarchy_octree_node = asset
            .hierarchy_node(node_id)
            .ok_or(OctreeServerError::HierarchyNodeNotFound)?
            .clone();

        let Some(loader) = self.loaders.get(&asset_id).cloned() else {
            return Err(OctreeServerError::LoaderNotFound);
        };
        let data = self.data.clone();

        #[allow(unused)]
        let task = spawn_async_task(async move {
            if let Err(err) = data
                .load_node_data_internal(asset_id, loader, &hierarchy_octree_node)
                .await
            {
                error!("An error occured in async task: {}", err);
            }
        });

        #[cfg(any(
            all(target_arch = "wasm32", not(feature = "wasm_worker")),
            not(feature = "multi_threaded")
        ))]
        task.detach();

        Ok(())
    }
}

/// A system that manages internal [`OctreeServer`] events, such as finalizing asset loads.
pub fn handle_internal_octree_events<T>(
    mut server: ResMut<OctreeServer<T>>,
    mut assets: ResMut<Assets<Octree<T>>>,
    mut load_tasks: ResMut<OctreeLoadTasks<T>>,
) where
    T: NodeData,
{
    // clone `server.data` because we need to borrow server as mutable in the loop
    for event in server.data.clone().octree_event_receiver.try_iter() {
        match event {
            InternalOctreeEvent::Loaded {
                id,
                loaded_asset,
                loader,
            } => {
                // store the asset in the assets resource
                assets
                    .insert(id, loaded_asset)
                    .expect("the AssetId is always valid");

                // store the loader in the server, this is where we need to borrow `server` as mutable
                server.loaders.insert(id, loader);

                info!("Loaded octree {:?}", id);
            }
            InternalOctreeEvent::SubHierarchyLoaded {
                id,
                node_id,
                hierarchy_nodes,
            } => {
                let key = (id, node_id);

                // update in flight hashset
                load_tasks.hierarchy_in_flight.remove(&key);

                let Some(octree) = assets.get_mut(id) else {
                    warn!(
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

                match octree.update_hierarchy_node(node_id, hierarchy_nodes[root_idx].clone()) {
                    Ok(_) => {
                        inserted_nodes[root_idx] = Some(node_id);
                    }
                    Err(error) => {
                        warn!("Unable to update hierarchy node: {:#}", error);
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

                    let new_id = match octree
                        .insert_hierarchy_node(Some(parent), hierarchy_nodes[idx].clone())
                    {
                        Ok(node_id) => node_id,
                        Err(error) => {
                            warn!("Unable to insert hierarchy node: {:#}", error);
                            continue;
                        }
                    };
                    inserted_nodes[idx] = Some(new_id);

                    for &child_idx in children[idx].iter().rev() {
                        stack.push((child_idx, new_id));
                    }
                }
            }
            InternalOctreeEvent::SubHierarchyLoadFailed {
                id,
                node_id,
                error: _,
            } => {
                let key = (id, node_id);

                // update in flight hashset
                load_tasks.hierarchy_in_flight.remove(&key);

                let Some(octree) = assets.get_mut(id) else {
                    warn!(
                        "No asset found for {:?}, unable to append loaded hierarchy nodes.",
                        id
                    );
                    continue;
                };

                let _ = octree.unset_node_hierarchy_loading(node_id);
            }
            InternalOctreeEvent::NodeDataLoaded {
                id,
                node_id,
                node_data,
            } => {
                let key = (id, node_id);

                // update in flight hashset
                load_tasks.node_in_flight.remove(&key);

                let Some(octree) = assets.get_mut(id) else {
                    warn!("No asset found for {:?}, unable to store node data.", id);
                    continue;
                };

                if let Err(error) = octree.insert_node_data(node_id, node_data) {
                    warn!(
                        "An error occured when adding node data to node {}/{:?}: {:#}",
                        id, node_id, error
                    );
                    continue;
                }
            }
            InternalOctreeEvent::NodeDataLoadFailed {
                id,
                node_id,
                error: _,
            } => {
                let key = (id, node_id);

                // update in flight hashset
                load_tasks.node_in_flight.remove(&key);

                let Some(octree) = assets.get_mut(id) else {
                    warn!(
                        "No asset found for {:?}, unable to append loaded hierarchy nodes.",
                        id
                    );
                    continue;
                };

                let _ = octree.unset_node_data_loading(node_id);
            }
        }
    }
}

#[derive(Clone, Debug, Error)]
pub enum OctreeServerError {
    #[error("Asset not found")]
    AssetNotFound,
    #[error("Loader not found")]
    LoaderNotFound,
    #[error("Hierarchy node not found")]
    HierarchyNodeNotFound,
}
