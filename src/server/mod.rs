mod byte_source;
mod eviction;
mod infos;
mod octree_loader;
mod task;

use std::{
    cmp::Reverse,
    sync::{Arc, PoisonError, RwLockReadGuard, RwLockWriteGuard},
};

use bevy::{
    app::{Plugin, PostUpdate},
    asset::{AssetHandleProvider, AssetId, Assets, Handle},
    camera::visibility::VisibilitySystems,
    ecs::{
        error::BevyError,
        resource::Resource,
        schedule::{IntoScheduleConfigs, SystemSet},
        system::{Commands, Res, ResMut},
        world::FromWorld,
    },
    log::{error, info},
    mesh::{Mesh, Mesh3d},
    platform::{collections::HashMap, sync::RwLock},
    prelude::{Deref, DerefMut},
    reflect::Reflect,
    tasks::IoTaskPool,
};
use crossbeam_channel::{Receiver, Sender};
use priority_queue::PriorityQueue;
use thiserror::Error;

use crate::{
    NodeId, PointCloud, PointCloudChunk, PointCloudInstances, PointCloudNode, PointCloudNodeKey,
    PointCloudNodeStatus, PointCloudVisibilitySystems,
};

pub use byte_source::*;
pub use eviction::*;
use infos::*;
pub use octree_loader::*;
pub use task::*;

/// A system set where commands accumulated when handling point cloud events are applied to the
/// world. So, depending on this system set will make sure that inserted components are available
/// (like Aabb for example).
///
/// [`Messages`]: bevy_ecs::message::Messages
#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct PointCloudServerSystems;

#[derive(Default)]
pub struct PointCloudServerPlugin {
    pub settings: PointCloudServerSettings,
}

impl Plugin for PointCloudServerPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.insert_resource(self.settings.clone())
            .init_resource::<PointCloudServer>()
            .init_resource::<PointCloudServerEvictionQueue>()
            .init_resource::<PointCloudTotalSize>()
            .init_resource::<PointCloudLoadTasks>()
            .init_resource::<PointCloudTracking>()
            .configure_sets(
                PostUpdate,
                // [`VisibilitySystems::CheckVisibility`] needs Aabb to do frustum culling
                PointCloudServerSystems.before(VisibilitySystems::CheckVisibility),
            )
            .add_systems(
                PostUpdate,
                (
                    cleanup_loaders,
                    (handle_internal_point_cloud_events, load_chunk_meshes)
                        .chain()
                        .in_set(PointCloudServerSystems),
                    process_point_cloud_load_tasks
                        .after(PointCloudVisibilitySystems::CheckPointCloudNodesVisibility),
                    update_point_cloud_server_node_eviction_queue
                        .after(PointCloudVisibilitySystems::CheckPointCloudNodesVisibility),
                ),
            );
    }
}

#[derive(Resource, Clone, Debug, Reflect)]
pub struct PointCloudServerSettings {
    pub max_size: usize,
    pub max_concurrent_hierarchy_load_task: usize,
    pub max_concurrent_chunks_load_task: usize,
}

impl Default for PointCloudServerSettings {
    fn default() -> Self {
        Self {
            max_size: 512 * 1024 * 1024, // 512 MB
            max_concurrent_hierarchy_load_task: 4,
            max_concurrent_chunks_load_task: 8,
        }
    }
}

#[derive(Error, Clone, Debug)]
pub enum PointCloudServerError {
    #[error("asset not found")]
    AssetNotFound,
    #[error("loader not found")]
    LoaderNotFound,
    #[error("hierarchy node not found")]
    HierarchyNodeNotFound,
    #[error("pointcloud is not an octree")]
    NotOctree,
}

#[derive(Resource, Clone)]
pub struct PointCloudServer {
    pub(crate) data: Arc<PointCloudServerData>,
}

impl FromWorld for PointCloudServer {
    fn from_world(world: &mut bevy::ecs::world::World) -> Self {
        let asset_server = world.resource::<Assets<PointCloud>>();
        let handle_provider = asset_server.get_handle_provider();

        let (event_sender, event_receiver) = crossbeam_channel::unbounded();

        Self {
            data: Arc::new(PointCloudServerData {
                infos: RwLock::new(PointCloudServerInfos::default()),
                loaders: Arc::new(RwLock::new(PointCloudServerLoaders(HashMap::new()))),
                handle_provider,
                event_sender,
                event_receiver,
            }),
        }
    }
}

impl PointCloudServer {
    /// Load a point cloud lazily (point cloud content will be loaded on the fly when needed)
    pub fn load<L: OctreeLoader>(&self, source: L::Source) -> Handle<PointCloud> {
        self.load_with_settings::<L>(source, |_| {})
    }

    pub fn load_with_settings<L: OctreeLoader>(
        &self,
        source: L::Source,
        settings: impl Fn(&mut L::Settings) + Send + Sync + 'static,
    ) -> Handle<PointCloud> {
        let mut default_settings = L::Settings::default();
        settings(&mut default_settings);

        let handle = self.data.handle_provider.reserve_handle().typed();
        let owned_handle = handle.clone();

        let data = self.data.clone();

        let task = IoTaskPool::get().spawn(async move {
            if let Err(err) = data
                .load_internal::<L>(source, default_settings, owned_handle)
                .await
            {
                error!("{}", err);
            }
        });

        #[cfg(not(any(target_arch = "wasm32", not(feature = "multi_threaded"))))]
        {
            let mut infos = self.write_infos();
            infos
                .pending_tasks
                .insert(TaskId::LoadInitialHierarchy(handle.id()), task);
        }

        #[cfg(any(target_arch = "wasm32", not(feature = "multi_threaded")))]
        task.detach();

        handle
    }

    fn load_sub_hierarchy(
        &mut self,
        asset_id: AssetId<PointCloud>,
        asset: &mut PointCloud,
        node_id: NodeId,
    ) -> Result<(), PointCloudServerError> {
        let hierarchy_node = asset
            .topology
            .as_octree_mut()
            .ok_or(PointCloudServerError::NotOctree)?
            .get_node_mut(node_id)
            .ok_or(PointCloudServerError::HierarchyNodeNotFound)?;

        hierarchy_node.status = PointCloudNodeStatus::Loading;

        let Some(loader) = ({
            let loaders = self.read_loaders();

            loaders.get(&asset_id).cloned()
        }) else {
            return Err(PointCloudServerError::LoaderNotFound);
        };

        let data = self.data.clone();
        let hierarchy_node = hierarchy_node.clone();

        let task = IoTaskPool::get().spawn(async move {
            if let Err(err) = data
                .load_sub_hierarchy_internal(asset_id, loader, &hierarchy_node)
                .await
            {
                error!(
                    "An error occured in async task when loading sub hierarchy: {:#}",
                    err
                );
            }
        });

        #[cfg(not(any(target_arch = "wasm32", not(feature = "multi_threaded"))))]
        {
            let mut infos = self.write_infos();
            infos.pending_tasks.insert(
                TaskId::LoadSubHierarchy(PointCloudNodeKey {
                    id: asset_id,
                    node_id,
                }),
                task,
            );
        }

        #[cfg(any(target_arch = "wasm32", not(feature = "multi_threaded")))]
        task.detach();

        Ok(())
    }

    /// TODO: make possible to patch the point count
    fn load_chunk(
        &mut self,
        asset_id: AssetId<PointCloud>,
        asset: &PointCloud,
        node_id: NodeId,
    ) -> Result<(), PointCloudServerError> {
        let hierarchy_node = asset
            .topology
            .as_octree()
            .ok_or(PointCloudServerError::NotOctree)?
            .get_node(node_id)
            .ok_or(PointCloudServerError::HierarchyNodeNotFound)?;

        // TODO add loading status somewhere ?

        let Some(loader) = ({
            let loaders = self.read_loaders();

            loaders.get(&asset_id).cloned()
        }) else {
            return Err(PointCloudServerError::LoaderNotFound);
        };

        let data = self.data.clone();
        let hierarchy_node = hierarchy_node.clone();

        let task = IoTaskPool::get().spawn(async move {
            if let Err(err) = data
                .load_chunk_internal(asset_id, loader, &hierarchy_node)
                .await
            {
                error!(
                    "An error occured in async task when loading node data: {:#}",
                    err
                );
            }
        });

        #[cfg(not(any(target_arch = "wasm32", not(feature = "multi_threaded"))))]
        {
            let mut infos = self.write_infos();
            infos.pending_tasks.insert(
                TaskId::LoadChunk(PointCloudNodeKey {
                    id: asset_id,
                    node_id,
                }),
                task,
            );
        }

        #[cfg(any(target_arch = "wasm32", not(feature = "multi_threaded")))]
        task.detach();

        Ok(())
    }

    pub(crate) fn write_infos(&self) -> RwLockWriteGuard<'_, PointCloudServerInfos> {
        self.data
            .infos
            .write()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn read_loaders(&self) -> RwLockReadGuard<'_, PointCloudServerLoaders> {
        self.data
            .loaders
            .read()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn write_loaders(&self) -> RwLockWriteGuard<'_, PointCloudServerLoaders> {
        self.data
            .loaders
            .write()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

#[derive(Deref, DerefMut)]
pub struct PointCloudServerLoaders(
    pub(crate) HashMap<AssetId<PointCloud>, Arc<dyn ErasedOctreeLoader>>,
);

/// Internal data used by [`PointCloudServer`]. This is intended to be used from within an [`Arc`].
pub(crate) struct PointCloudServerData {
    pub(crate) infos: RwLock<PointCloudServerInfos>,
    pub(crate) loaders: Arc<RwLock<PointCloudServerLoaders>>,
    pub(crate) handle_provider: AssetHandleProvider,
    pub(crate) event_sender: Sender<InternalPointCloudEvent>,
    pub(crate) event_receiver: Receiver<InternalPointCloudEvent>,
}

impl PointCloudServerData {
    async fn load_internal<L: OctreeLoader>(
        &self,
        source: L::Source,
        settings: L::Settings,
        handle: Handle<PointCloud>,
    ) -> Result<(), BevyError> {
        let loader = L::from_source(source, settings).await.map_err(Into::into)?;

        let asset_id = handle.id();

        let mut point_cloud = PointCloud::new_octree();

        // Safe to unwrap because it has been instantiated above as octree.
        let octree_topology = point_cloud.topology.as_octree_mut().unwrap();

        let mut builder = OctreeHierarchyBuilder::new();

        let metadata = loader.load_metadata().await.map_err(Into::into)?;
        point_cloud.aabb = metadata.aabb;
        point_cloud.spacing = metadata.spacing;

        loader
            .load_initial_hierarchy(&mut builder)
            .await
            .map_err(Into::into)?;

        let octree_hierarchy: ErasedOctreeHierarchy = builder.into();

        let Some(root) = octree_hierarchy.get_root() else {
            return Err(BevyError::from(
                "Loaded point cloud hierarchy is empty or missing a root node",
            ));
        };

        if point_cloud.aabb.is_none()
            && let Some(aabb) = root.aabb
        {
            point_cloud.aabb = Some(aabb);
        }

        append_hierarchy(octree_topology, octree_hierarchy, None)?;

        info!("Send event InternalPointCloudEvent::Loaded");
        self.event_sender
            .send(InternalPointCloudEvent::Loaded {
                id: asset_id,
                loaded_asset: point_cloud,
                loader: Arc::new(loader),
            })
            .expect("Failed to send internal point cloud event");

        Ok(())
    }

    async fn load_sub_hierarchy_internal(
        &self,
        id: AssetId<PointCloud>,
        loader: Arc<dyn ErasedOctreeLoader>,
        hierarchy_node: &PointCloudNode,
    ) -> Result<(), BevyError> {
        match loader.load_sub_hierarchy(hierarchy_node).await {
            Ok(hierarchy_nodes) => {
                self.event_sender
                    .send(InternalPointCloudEvent::SubHierarchyLoaded {
                        id,
                        node_id: hierarchy_node.id,
                        octree_hierarchy: hierarchy_nodes,
                    })
                    .expect("Failed to send internal point cloud server event");
            }
            Err(err) => {
                error!("Error loading sub hierarchy: {}", err);
                self.event_sender
                    .send(InternalPointCloudEvent::SubHierarchyLoadFailed {
                        id,
                        node_id: hierarchy_node.id,
                        error: err.to_string(),
                    })
                    .expect("Failed to send internal point cloud server event");
            }
        };

        Ok(())
    }

    async fn load_chunk_internal(
        &self,
        id: AssetId<PointCloud>,
        loader: Arc<dyn ErasedOctreeLoader>,
        hierarchy_node: &PointCloudNode,
    ) -> Result<(), BevyError> {
        match loader.load_chunk(hierarchy_node).await {
            Ok(result) => self
                .event_sender
                .send(InternalPointCloudEvent::ChunkLoaded {
                    id,
                    node_id: hierarchy_node.id,
                    result,
                })
                .expect("Failed to send internal point cloud server event"),
            Err(err) => {
                error!("Error loading node data: {}", err);
                self.event_sender
                    .send(InternalPointCloudEvent::ChunkLoadFailed {
                        id,
                        node_id: hierarchy_node.id,
                        error: err.to_string(),
                    })
                    .expect("Failed to send internal point cloud server event");
            }
        }

        Ok(())
    }
}

/// Internal events for asset load results
pub(crate) enum InternalPointCloudEvent {
    Loaded {
        id: AssetId<PointCloud>,
        loaded_asset: PointCloud,
        loader: Arc<dyn ErasedOctreeLoader>,
    },
    SubHierarchyLoaded {
        id: AssetId<PointCloud>,
        node_id: NodeId,
        octree_hierarchy: ErasedOctreeHierarchy,
    },
    SubHierarchyLoadFailed {
        id: AssetId<PointCloud>,
        node_id: NodeId,
        error: String,
    },
    ChunkLoaded {
        id: AssetId<PointCloud>,
        node_id: NodeId,
        result: ChunkLoadResult,
    },
    ChunkLoadFailed {
        id: AssetId<PointCloud>,
        node_id: NodeId,
        error: String,
    },
}

/// This resource contains a priority queue to determine which nodes to evict first.
/// Nodes that are seen less recently are first in this queue.
#[derive(Resource, Default)]
pub struct PointCloudEvictionQueue {
    pub eviction_queue: PriorityQueue<PointCloudNodeKey, Reverse<u128>>,
}

/// Loads the chunk's meshes in the meshes asset, so later in the GPU.
/// Then add [`Mesh3d`] to all chunks using it.
fn load_chunk_meshes(
    mut meshes: ResMut<Assets<Mesh>>,
    mut chunks: ResMut<Assets<PointCloudChunk>>,
    point_cloud_instances: Res<PointCloudInstances>,
    mut point_cloud_total_size: ResMut<PointCloudTotalSize>,
    mut tracking: ResMut<PointCloudTracking>,
    mut commands: Commands,
) {
    for (key, mesh) in tracking.loaded_chunks.drain(..) {
        let Some(mut chunk) = chunks.get_mut(key.chunk_id) else {
            continue;
        };
        let mesh_handle = meshes.add(mesh.clone());
        point_cloud_total_size.total_size += chunk.vertex_buffer_size;

        chunk.mesh_handle = Some(mesh_handle.clone());

        // Spawn [`Mesh3d`] entity only for the root chunk, so we have a
        // [`RenderMeshInstanceCpu`] or [`RenderMeshInstanceGpu`] available later.
        if chunk.depth == 0
            && let Some(point_cloud_entities) = point_cloud_instances.get(&key.id)
        {
            for (_, chunks) in point_cloud_entities.iter() {
                if let Some(&chunk_entity) = chunks.get(&key.chunk_id) {
                    commands
                        .entity(chunk_entity)
                        .insert(Mesh3d(mesh_handle.clone()));
                }
            }
        }
    }
}
