#![expect(missing_docs, reason = "Not all docs are written yet.")]

use std::path::PathBuf;

use bevy::{
    app::{App, Last, Plugin},
    asset::{AssetApp, AssetEvent, AssetPath, Assets},
    ecs::{
        entity::Entity,
        hierarchy::ChildOf,
        lifecycle::HookContext,
        message::MessageReader,
        query::Added,
        system::{Commands, Query, Res},
        world::DeferredWorld,
    },
    log::{info, warn},
    mesh::Mesh3d,
    platform::collections::HashMap,
    render::batching::NoAutomaticBatching,
    shader::{load_shader_library, ShaderRef},
};

mod components;
#[cfg(feature = "server")]
mod loader;
mod materials;
mod point_cloud;
pub mod prelude;
mod render;
mod resources;
#[cfg(feature = "server")]
mod server;
mod visibility;

pub use components::*;
#[cfg(feature = "server")]
pub use loader::*;
pub use materials::*;
pub use point_cloud::*;
pub use render::*;
pub use resources::*;
#[cfg(feature = "server")]
pub use server::*;
pub use visibility::*;

#[cfg(feature = "las")]
use crate::las::LasLoaderPlugin;

#[derive(Default)]
pub struct PointCloudPlugin {
    #[cfg(feature = "server")]
    settings: PointCloudServerSettings,
}

impl Plugin for PointCloudPlugin {
    fn build(&self, app: &mut App) {
        load_shader_library!(app, "assets/shaders/forward_io.wgsl");
        load_shader_library!(app, "assets/shaders/functions.wgsl");
        load_shader_library!(app, "assets/shaders/pointcloud_types.wgsl");
        load_shader_library!(app, "assets/shaders/pointcloud_bindings.wgsl");
        load_shader_library!(app, "assets/shaders/pointcloud_functions.wgsl");

        app.init_asset::<PointCloud>()
            .init_asset::<PointCloudChunk>()
            .register_asset_reflect::<PointCloud>()
            .register_asset_reflect::<PointCloudChunk>()
            .init_resource::<PointCloudInstances>();

        app.world_mut()
            .register_component_hooks::<PointCloud3d>()
            .on_insert(on_insert_point_cloud_3d)
            .on_discard(on_discard_point_cloud_3d);

        app.world_mut()
            .register_component_hooks::<PointCloudChunk3d>()
            .on_insert(on_insert_point_cloud_chunk_3d)
            .on_discard(on_discard_point_cloud_chunk_3d);

        #[cfg(feature = "server")]
        app.add_plugins(PointCloudServerPlugin {
            settings: self.settings.clone(),
        });
        app.add_plugins(PointCloudVisiblityPlugin);
        app.add_plugins(RenderPointCloudPlugin);
        app.add_plugins(SimplePointCloudMaterialPlugin);
        app.add_plugins(StandardPointCloudMaterialPlugin);

        app.add_systems(
            Last,
            (
                spawn_point_cloud_chunks,
                spawn_splat_point_cloud_chunks_from_asset_loader,
            ),
        );

        #[cfg(feature = "las")]
        app.add_plugins(LasLoaderPlugin);

        // prevents automatic batching of point clouds (for transparent phases)
        app.register_required_components::<PointCloud3d, NoAutomaticBatching>();
    }
}

pub fn on_insert_point_cloud_3d(
    mut world: DeferredWorld<'_>,
    HookContext { entity, .. }: HookContext,
) {
    // then add point cloud asset tracking
    if let Some(PointCloud3d(handle)) = world.get::<PointCloud3d>(entity).cloned() {
        let mut instances = world.resource_mut::<PointCloudInstances>();
        let entities = instances.entry(handle.id()).or_default();
        entities.insert(entity, PointCloudChunks::default());
        // info!("on_insert_point_cloud_3d: {:#?}", instances);
    }
}

pub fn on_discard_point_cloud_3d(
    mut world: DeferredWorld<'_>,
    HookContext { entity, .. }: HookContext,
) {
    // remove previous point cloud asset tracking
    if let Some(PointCloud3d(handle)) = world.get::<PointCloud3d>(entity).cloned() {
        let mut instances = world.resource_mut::<PointCloudInstances>();
        let entities = instances.entry(handle.id()).or_default();
        entities.remove(&entity);

        // info!("on_discard_point_cloud_3d: {:#?}", instances);
    }
}

pub fn on_insert_point_cloud_chunk_3d(
    mut world: DeferredWorld<'_>,
    HookContext { entity, .. }: HookContext,
) {
    // add point cloud asset tracking
    if let (Some(PointCloudChunk3d(chunk_handle)), maybe_parent) = (
        world.get::<PointCloudChunk3d>(entity).cloned(),
        world.get::<ChildOf>(entity),
    ) {
        let point_cloud_entity = {
            if let Some(&ChildOf(parent_entity)) = maybe_parent
                && world.entity(parent_entity).contains::<PointCloud3d>()
            {
                parent_entity
            } else {
                entity
            }
        };
        if let Some(PointCloud3d(point_cloud_handle)) =
            world.get::<PointCloud3d>(point_cloud_entity).cloned()
        {
            let mut instances = world.resource_mut::<PointCloudInstances>();
            let entities = instances.entry(point_cloud_handle.id()).or_default();
            let chunk_entities = entities.entry(point_cloud_entity).or_default();
            chunk_entities.insert(chunk_handle.id(), entity);

            // info!("on_insert_point_cloud_chunk_3d: {:#?}", instances);
        } else {
            warn!("on_insert_point_cloud_chunk_3d: PointCloud3d entity not found.");
        }
    } else {
        warn!(
            "on_insert_point_cloud_chunk_3d: some entities not found for entity {:?}",
            entity
        );
    }
}

pub fn on_discard_point_cloud_chunk_3d(
    mut world: DeferredWorld<'_>,
    HookContext { entity, .. }: HookContext,
) {
    // remove previous point cloud asset tracking
    if let (Some(PointCloudChunk3d(chunk_handle)), maybe_parent) = (
        world.get::<PointCloudChunk3d>(entity).cloned(),
        world.get::<ChildOf>(entity),
    ) {
        let point_cloud_entity = {
            if let Some(&ChildOf(parent_entity)) = maybe_parent {
                if let Ok(parent) = world.get_entity(parent_entity) {
                    if parent.contains::<PointCloud3d>() {
                        parent_entity
                    } else {
                        entity
                    }
                } else {
                    // the entity has been despawned, return early
                    return;
                }
            } else {
                entity
            }
        };

        if let Some(PointCloud3d(point_cloud_handle)) =
            world.get::<PointCloud3d>(point_cloud_entity).cloned()
        {
            let mut instances = world.resource_mut::<PointCloudInstances>();
            let entities = instances.entry(point_cloud_handle.id()).or_default();
            let chunk_entities = entities.entry(point_cloud_entity).or_default();
            chunk_entities.insert(chunk_handle.id(), entity);

            let mut instances = world.resource_mut::<PointCloudInstances>();
            let entities = instances.entry(point_cloud_handle.id()).or_default();
            let chunk_entities = entities.entry(point_cloud_entity).or_default();
            chunk_entities.remove(&chunk_handle.id());

            // info!("on_discard_point_cloud_chunk_3d: {:#?}", instances);
        } else {
            warn!("on_insert_point_cloud_chunk_3d: PointCloud3d entity not found.");
        }
    } else {
        warn!(
            "on_discard_point_cloud_chunk_3d: some entities not found for entity {:?}",
            entity
        );
    }
}

/// This system spawn point cloud chunks, when a point cloud 3d is spawned.
/// Chunks are spawned as direct child of the point cloud 3d, but are hierarchicaly organised
/// between themselves using [`ChildChunkOf`] relation.
pub fn spawn_point_cloud_chunks(
    mut commands: Commands,
    point_clouds: Res<Assets<PointCloud>>,
    point_cloud_chunks: Res<Assets<PointCloudChunk>>,
    query: Query<(Entity, &PointCloud3d), Added<PointCloud3d>>,
) {
    // Map to associate SlotMap's NodeId with Bevy's Entity.
    // Preserve allocations
    let mut node_to_entity = HashMap::new();

    for (root_entity, point_cloud_comp) in &query {
        node_to_entity.clear();
        let Some(point_cloud) = point_clouds.get(point_cloud_comp) else {
            continue;
        };
        let Some(octree) = point_cloud.topology.as_octree() else {
            continue;
        };

        node_to_entity.reserve(octree.nodes.len());

        for node in octree.iter_loaded_chunks() {
            if let Some(chunk_handle) = &node.chunk {
                // Link to the chunk to its parent chunk if any, and always to the point cloud
                let chunk_entity = if let Some(parent_id) = node.parent_id
                    && let Some(&parent_entity) = node_to_entity.get(&parent_id)
                {
                    commands
                        .spawn((
                            PointCloudChunk3d(chunk_handle.clone()),
                            ChildOf(root_entity),
                            ChildChunkOf(parent_entity),
                        ))
                        .id()
                } else {
                    // Or directly on the point cloud root
                    commands
                        .entity(root_entity)
                        .insert(PointCloudChunk3d(chunk_handle.clone()));

                    root_entity
                };
                node_to_entity.insert(node.id, chunk_entity);

                // Spawn [`Mesh3d`] entity only for the root chunk, so we have a
                // [`RenderMeshInstanceCpu`] or [`RenderMeshInstanceGpu`] available later.
                if node.depth == 0 {
                    // Set the aabb if any
                    if let Some(aabb) = node.aabb {
                        info!("spawn aabb for chunk {:?}", chunk_entity);
                        commands.entity(chunk_entity).insert(aabb);
                    }
                    // Now look for the mesh
                    if let Some(chunk) = point_cloud_chunks.get(chunk_handle.id())
                        && let Some(mesh_handle) = &chunk.mesh_handle
                    {
                        commands
                            .entity(chunk_entity)
                            .insert(Mesh3d(mesh_handle.clone()));
                    }
                }
            }
        }
    }
}

fn spawn_splat_point_cloud_chunks_from_asset_loader(
    mut events: MessageReader<AssetEvent<PointCloud>>,
    point_clouds: Res<Assets<PointCloud>>,
    point_cloud_chunks: Res<Assets<PointCloudChunk>>,
    point_cloud_instances: Res<PointCloudInstances>,
    mut commands: Commands,
) {
    for event in events.read() {
        match event {
            AssetEvent::Added { id } => {
                info!("PointCloud added: {:?}", id);
            }
            AssetEvent::LoadedWithDependencies { id } => {
                info!("PointCloud loaded with dependencies: {:?}", id);

                let Some(point_cloud) = point_clouds.get(*id) else {
                    continue;
                };

                let Some(instances) = point_cloud_instances.get(id) else {
                    continue;
                };

                let Some(chunk_handle) = point_cloud.topology.as_flat() else {
                    continue;
                };

                let Some(chunk) = point_cloud_chunks.get(chunk_handle) else {
                    continue;
                };

                // TODO handle the case where the mesh handle is not yet available ?
                let Some(mesh_handle) = chunk.mesh_handle.as_ref() else {
                    continue;
                };

                let Some(aabb) = chunk.aabb.as_ref() else {
                    continue;
                };

                let insert_batch = instances
                    .keys()
                    .map(|&entity| {
                        (
                            entity,
                            (
                                PointCloudChunk3d(chunk_handle.clone()),
                                Mesh3d(mesh_handle.clone()),
                                *aabb,
                            ),
                        )
                    })
                    .collect::<Vec<_>>();

                commands.insert_batch(insert_batch);
            }
            AssetEvent::Modified { id: _ } => {
                // info!("PointCloud modified: {:?}", id);
            }
            AssetEvent::Removed { id: _ } => {
                // info!("PointCloud removed: {:?}", id);
            }
            AssetEvent::Unused { id: _ } => {
                // info!("PointCloud unused: {:?}", id);
            }
        }
    }
}

fn shader_ref(path: PathBuf) -> ShaderRef {
    ShaderRef::Path(AssetPath::from_path_buf(path).with_source("embedded"))
}
