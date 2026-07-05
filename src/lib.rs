#![expect(missing_docs, reason = "Not all docs are written yet.")]

use bevy::{
    app::{App, Plugin},
    asset::{embedded_asset, AssetApp},
    ecs::{hierarchy::ChildOf, lifecycle::HookContext, world::DeferredWorld},
    log::warn,
    shader::load_shader_library,
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
        load_shader_library!(app, "assets/shaders/material_types.wgsl");
        load_shader_library!(app, "assets/shaders/simple_material_types.wgsl");
        load_shader_library!(app, "assets/shaders/simple_material_bindings.wgsl");
        embedded_asset!(app, "assets/shaders/pointcloud.wgsl");
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
        app.add_plugins(MaterialPlugin::<SimplePointCloudMaterial>::default());
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
