use super::asset::NewOctree;
use super::hierarchy::{
    HierarchyNode, HierarchyNodeData, HierarchyNodeStatus, HierarchyOctreeNode,
};
use super::loader::resources::OctreeLoadTasks;
use super::loader::OctreeLoader;
use super::node::{NodeData, NodeStatus, OctreeNode};
use crate::octree::storage::NodeId;
use bevy_asset::{AssetHandleProvider, AssetId, Assets, Handle};
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use bevy_log::prelude::*;
use bevy_platform::collections::HashMap;
use bevy_tasks::IoTaskPool;
use crossbeam::channel::{Receiver, Sender};
use std::sync::Arc;
use thiserror::Error;

#[derive(Resource)]
pub struct OctreeServer<L, H, T>
where
    L: OctreeLoader<H, T> + 'static,
    H: HierarchyNodeData,
    T: NodeData,
{
    pub(crate) data: Arc<OctreeServerData<L, H, T>>,
    pub(crate) loaders: HashMap<AssetId<NewOctree<H, T>>, Arc<L>>,
}

// Manually implement clone to prevent adding bounds
// impl<L, H, T> Clone for OctreeServer<L, H, T>
// where
//     L: OctreeLoader<H> + 'static,
//     H: Send + Sync + TypePath,
//     T: NodeData,
// {
//     fn clone(&self) -> Self {
//         Self {
//             data: self.data.clone(),
//             loaders: self.loaders.clone(),
//         }
//     }
// }

pub struct OctreeServerData<L, H, T>
where
    L: OctreeLoader<H, T>,
    H: HierarchyNodeData,
    T: NodeData,
{
    pub(crate) handle_provider: AssetHandleProvider,
    pub(crate) octree_event_sender: Sender<InternalOctreeEvent<L, H, T>>,
    pub(crate) octree_event_receiver: Receiver<InternalOctreeEvent<L, H, T>>,
}

impl<L, H, T> OctreeServerData<L, H, T>
where
    L: OctreeLoader<H, T> + 'static,
    H: HierarchyNodeData,
    T: NodeData,
{
    async fn load_internal(
        &self,
        url: &str,
        handle: Handle<NewOctree<H, T>>,
    ) -> Result<(), BevyError> {
        let loader = L::from_url(url).await.map_err(|error| error.into())?;
        let asset_id = handle.id();

        let mut octree = NewOctree::<H, T>::new();

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
            parents.push(octree.insert_hierarchy_node(parent_id, node)?);
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
        id: AssetId<NewOctree<H, T>>,
        loader: Arc<L>,
        hierarchy_node: &HierarchyOctreeNode<H>,
    ) -> Result<(), BevyError> {
        let hierarchy_nodes = loader
            .load_hierarchy(&hierarchy_node)
            .await
            .map_err(|error| error.into())?;

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
        id: AssetId<NewOctree<H, T>>,
        loader: Arc<L>,
        hierarchy_node: &HierarchyOctreeNode<H>,
    ) -> Result<(), BevyError> {
        let node_data = loader
            .load_node_data(&hierarchy_node)
            .await
            .map_err(|error| error.into())?;

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
pub(crate) enum InternalOctreeEvent<L, H, T>
where
    L: OctreeLoader<H, T>,
    H: HierarchyNodeData,
    T: NodeData,
{
    Loaded {
        id: AssetId<NewOctree<H, T>>,
        loaded_asset: NewOctree<H, T>,
        loader: Arc<L>,
    },
    SubHierarchyLoaded {
        id: AssetId<NewOctree<H, T>>,
        node_id: NodeId,
        hierarchy_nodes: Vec<HierarchyNode<H>>,
    },
    NodeDataLoaded {
        id: AssetId<NewOctree<H, T>>,
        node_id: NodeId,
        node_data: T,
    },
}

impl<L, H, T> FromWorld for OctreeServer<L, H, T>
where
    L: OctreeLoader<H, T>,
    H: HierarchyNodeData,
    T: NodeData,
{
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<Assets<NewOctree<H, T>>>();
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

impl<L, H, T> OctreeServer<L, H, T>
where
    L: OctreeLoader<H, T> + 'static,
    H: HierarchyNodeData,
    T: NodeData,
{
    /// Load an octree lazily (octree content will be loaded on the fly when needed)
    pub fn load_octree(&self, url: String) -> Handle<NewOctree<H, T>> {
        let handle = self.data.handle_provider.reserve_handle().typed();
        let owned_handle = handle.clone();

        let data = self.data.clone();
        let task = IoTaskPool::get().spawn(async move {
            if let Err(err) = data.load_internal(&url, owned_handle).await {
                error!("{}", err);
            }
        });

        #[cfg(any(target_arch = "wasm32", not(feature = "multi_threaded")))]
        task.detach();

        handle
    }

    pub fn load_sub_hierarchy(
        &mut self,
        asset_id: AssetId<NewOctree<H, T>>,
        asset: &mut NewOctree<H, T>,
        node_id: NodeId,
    ) -> Result<(), OctreeServerError> {
        let hierarchy_octree_node_mut = asset
            .hierarchy_node_mut(node_id)
            .ok_or(OctreeServerError::HierarchyNodeNotFound)?;

        hierarchy_octree_node_mut.status = HierarchyNodeStatus::Loading;

        let hierarchy_octree_node = hierarchy_octree_node_mut.clone();

        let Some(loader) = self.loaders.get(&asset_id).cloned() else {
            return Err(OctreeServerError::LoaderNotFound);
        };
        let data = self.data.clone();

        let task = IoTaskPool::get().spawn(async move {
            if let Err(err) = data
                .load_sub_hierarchy_internal(asset_id, loader, &hierarchy_octree_node)
                .await
            {
                error!("{}", err);
            }
        });

        #[cfg(any(target_arch = "wasm32", not(feature = "multi_threaded")))]
        task.detach();

        Ok(())
    }

    pub fn load_node_data(
        &mut self,
        asset_id: AssetId<NewOctree<H, T>>,
        asset: &mut NewOctree<H, T>,
        node_id: NodeId,
    ) -> Result<(), OctreeServerError> {
        let node_mut = asset
            .node_mut(node_id)
            .ok_or(OctreeServerError::HierarchyNodeNotFound)?;

        node_mut.status = NodeStatus::Loading;

        let hierarchy_octree_node = node_mut.hierarchy.clone();

        let Some(loader) = self.loaders.get(&asset_id).cloned() else {
            return Err(OctreeServerError::LoaderNotFound);
        };
        let data = self.data.clone();

        let task = IoTaskPool::get().spawn(async move {
            if let Err(err) = data
                .load_node_data_internal(asset_id, loader, &hierarchy_octree_node)
                .await
            {
                error!("{}", err);
            }
        });

        #[cfg(any(target_arch = "wasm32", not(feature = "multi_threaded")))]
        task.detach();

        Ok(())
    }
}

/// A system that manages internal [`OctreeServer`] events, such as finalizing asset loads.
pub fn handle_internal_octree_events<L, H, T>(
    mut server: ResMut<OctreeServer<L, H, T>>,
    mut assets: ResMut<Assets<NewOctree<H, T>>>,
    mut load_tasks: ResMut<OctreeLoadTasks<H, T>>,
) where
    L: OctreeLoader<H, T> + 'static,
    H: HierarchyNodeData,
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
                info!("Loaded {} new hierarchy nodes", parents.len());
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

                let Some(node) = octree.node_mut(node_id) else {
                    warn!(
                        "No node found for asset {:?} and node id {:?}, unable to store node data.",
                        id, node_id,
                    );
                    continue;
                };
                node.data = Some(node_data);
                node.status = NodeStatus::Loaded;
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

#[derive(SystemParam)]
pub struct OctreeServerHelper<'w, L, H, T>
where
    L: OctreeLoader<H, T>,
    H: HierarchyNodeData,
    T: NodeData,
{
    pub(crate) assets: ResMut<'w, Assets<NewOctree<H, T>>>,
    server: Res<'w, OctreeServer<L, H, T>>,
}

impl<'w, L, H, T> OctreeServerHelper<'w, L, H, T>
where
    L: OctreeLoader<H, T>,
    H: HierarchyNodeData,
    T: NodeData,
{
    pub fn load_octree(&self, url: String) -> Handle<NewOctree<H, T>> {
        self.server.load_octree(url)
    }

    pub fn load_sub_hierarchy(
        &mut self,
        asset_id: impl Into<AssetId<NewOctree<H, T>>>,
        node_id: NodeId,
    ) -> Result<(), OctreeServerError> {
        let asset_id = asset_id.into();
        let Some(asset) = self.assets.get_mut(asset_id) else {
            return Err(OctreeServerError::AssetNotFound);
        };
        let hierarchy_octree_node_mut = asset
            .hierarchy_node_mut(node_id)
            .ok_or(OctreeServerError::HierarchyNodeNotFound)?;

        hierarchy_octree_node_mut.status = HierarchyNodeStatus::Loading;

        let hierarchy_octree_node = hierarchy_octree_node_mut.clone();

        let Some(loader) = self.server.loaders.get(&asset_id).cloned() else {
            return Err(OctreeServerError::LoaderNotFound);
        };
        let data = self.server.data.clone();

        let task = IoTaskPool::get().spawn(async move {
            if let Err(err) = data
                .load_sub_hierarchy_internal(asset_id, loader, &hierarchy_octree_node)
                .await
            {
                error!("{}", err);
            }
        });

        #[cfg(any(target_arch = "wasm32", not(feature = "multi_threaded")))]
        task.detach();

        Ok(())
    }
}
