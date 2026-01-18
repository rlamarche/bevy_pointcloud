pub mod process;
pub mod resources;
mod task;

use super::asset::Octree;
use super::hierarchy::{HierarchyNode, HierarchyNodeStatus, HierarchyOctreeNode};
use super::loader::{ErasedOctreeLoader, OctreeLoader};
use super::node::{NodeData, NodeStatus};
use super::visibility::CheckOctreeNodesVisibility;
use crate::octree::server::task::spawn_async_task;
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
use std::marker::PhantomData;
use std::sync::Arc;
use thiserror::Error;

pub struct OctreeServerPlugin<T, C, A>(PhantomData<fn() -> (T, C, A)>);

impl<T, C, A> Default for OctreeServerPlugin<T, C, A> {
    fn default() -> Self {
        OctreeServerPlugin(PhantomData)
    }
}
impl<T, C, A> Plugin for OctreeServerPlugin<T, C, A>
where
    T: NodeData,
    C: Component,
    for<'a> &'a C: Into<AssetId<Octree<T>>>,
    A: Send + Sync + 'static,
{
    fn build(&self, app: &mut App) {
        app.init_resource::<OctreeServer<T>>().add_systems(
            PostUpdate,
            (
                handle_internal_octree_events::<T>,
                process_octree_load_tasks::<T>.after(CheckOctreeNodesVisibility),
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
            .map_err(|error| error.into())?;

        let mut parents = Vec::with_capacity(initial_hierarchy.len());
        for node in initial_hierarchy {
            let mut parent_id = None;
            if let Some(parent) = node.parent_id {
                parent_id = Some(parents[parent]);
            }
            parents.push(octree.insert_hierarchy_node(parent_id, node.into())?);
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
        let loaded_hierarchy_nodes = loader.load_hierarchy(hierarchy_node).await?;

        let hierarchy_nodes = loaded_hierarchy_nodes.into_iter().map(Into::into).collect();

        self.octree_event_sender
            .send(InternalOctreeEvent::SubHierarchyLoaded {
                id,
                node_id: hierarchy_node.id,
                hierarchy_nodes,
            })
            .expect("Failed to send internal octree event");

        Ok(())
    }

    async fn load_node_data_internal(
        &self,
        id: AssetId<Octree<T>>,
        loader: Arc<dyn ErasedOctreeLoader<T>>,
        hierarchy_node: &HierarchyOctreeNode,
    ) -> Result<(), BevyError> {
        let node_data = loader.load_node_data(hierarchy_node).await?;

        self.octree_event_sender
            .send(InternalOctreeEvent::NodeDataLoaded {
                id,
                node_id: hierarchy_node.id,
                node_data,
            })
            .expect("Failed to send internal octree event");

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
    NodeDataLoaded {
        id: AssetId<Octree<T>>,
        node_id: NodeId,
        node_data: T,
    },
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

                // this vec will store each inserted nodes
                let mut parents = Vec::with_capacity(hierarchy_nodes.len());

                // insert each new hierarchy node in the asset
                for node in hierarchy_nodes {
                    if let Some(parent) = node.parent_id {
                        // get the corresponding parent's node id from the indexed vec
                        let parent_id = Some(parents[parent]);

                        match octree.insert_hierarchy_node(parent_id, node) {
                            Ok(node_id) => parents.push(node_id),
                            Err(error) => {
                                warn!("Unable to insert hierarchy node: {:#}", error);
                                break;
                            }
                        };
                    } else {
                        // this is the first node, it exists already, just update it
                        match octree.update_hierarchy_node(node_id, node) {
                            Ok(_) => {
                                parents.push(node_id);
                            }
                            Err(error) => {
                                warn!("Unable to update hierarchy node: {:#}", error);
                                break;
                            }
                        }
                    }
                }
                // info!("Loaded {} new hierarchy nodes", parents.len());
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
