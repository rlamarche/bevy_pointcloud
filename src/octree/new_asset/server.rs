use super::loader::OctreeLoader;
use crate::octree::asset::Octree;
use crate::octree::new_asset::asset::NewOctree;
use bevy_asset::{AssetContainer, AssetHandleProvider, AssetId, Assets, Handle};
use bevy_ecs::prelude::*;
use bevy_log::prelude::*;
use bevy_platform::collections::HashMap;
use bevy_reflect::TypePath;
use bevy_tasks::IoTaskPool;
use crossbeam::channel::{Receiver, Sender};
use std::sync::Arc;
use crate::octree::new_asset::hierarchy::HierarchyNodeData;

#[derive(Resource)]
pub struct OctreeServer<L, H, T>
where
    L: OctreeLoader<H> + 'static,
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
{
    pub(crate) data: Arc<OctreeServerData<L, H, T>>,
    pub(crate) loaders: HashMap<AssetId<NewOctree<H, T>>, Arc<L>>,
}

// Manually implement clone to prevent adding bounds
// impl<L, H, T> Clone for OctreeServer<L, H, T>
// where
//     L: OctreeLoader<H> + 'static,
//     H: Send + Sync + TypePath,
//     T: Send + Sync + TypePath,
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
    L: OctreeLoader<H> + 'static,
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
{
    pub(crate) handle_provider: AssetHandleProvider,
    pub(crate) octree_event_sender: Sender<InternalOctreeEvent<L, H, T>>,
    pub(crate) octree_event_receiver: Receiver<InternalOctreeEvent<L, H, T>>,
}

impl<L, H, T> OctreeServerData<L, H, T>
where
    L: OctreeLoader<H> + 'static,
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
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
}

/// Internal events for asset load results
pub(crate) enum InternalOctreeEvent<L, H, T>
where
    L: OctreeLoader<H>,
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
{
    Loaded {
        id: AssetId<NewOctree<H, T>>,
        loaded_asset: NewOctree<H, T>,
        loader: Arc<L>,
    },
}

impl<L, H, T> FromWorld for OctreeServer<L, H, T>
where
    L: OctreeLoader<H> + 'static,
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
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
    L: OctreeLoader<H> + 'static,
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
{
    /// Load an octree lazily (octree content will be loaded on the fly when needed)
    pub fn load_octree(&self, url: String) -> Result<Handle<NewOctree<H, T>>, L::Error> {
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

        Ok(handle)
    }
}

/// A system that manages internal [`OctreeServer`] events, such as finalizing asset loads.
pub fn handle_internal_octree_events<L, H, T>(
    mut server: ResMut<OctreeServer<L, H, T>>,
    mut assets: ResMut<Assets<NewOctree<H, T>>>,
) where
    L: OctreeLoader<H> + 'static,
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
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
        }
    }
}
