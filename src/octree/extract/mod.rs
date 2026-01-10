pub mod limiter;
pub mod prepare;
pub mod render_asset;
pub mod resources;

use super::asset::Octree;
pub(crate) use super::extract::resources::{PrepareNextFrameOctreeNodes, RenderOctrees};
use super::hierarchy::HierarchyNodeData;
use super::node::{NodeData, OctreeNode};
use crate::octree::extract::render_asset::RenderOctreeNodeData;
pub(crate) use crate::octree::visibility::components::{
    OctreesVisibility, VisibleOctreeNode,
};
use crate::point_cloud_material::PointCloudMaterial3d;
use bevy_app::prelude::*;
use bevy_asset::{AssetId, Assets};
use bevy_camera::visibility::ViewVisibility;
use bevy_camera::Camera;
use bevy_ecs::prelude::*;
use bevy_ecs::query::{QueryFilter, QueryItem, ReadOnlyQueryData};
use bevy_ecs::schedule::ScheduleConfigs;
use bevy_ecs::system::ScheduleSystem;
use bevy_log::prelude::*;
use bevy_platform::collections::hash_map::Entry;
use bevy_platform::collections::HashMap;
use bevy_reflect::TypePath;
use bevy_render::camera::extract_cameras;
use bevy_render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy_render::sync_world::RenderEntity;
use bevy_render::{Extract, ExtractSchedule, Render, RenderApp, RenderSystems};
use limiter::{
    extract_render_asset_bytes_per_frame, reset_render_asset_bytes_per_frame,
    RenderOctreeNodesBytesPerFrame, RenderOctreeNodesBytesPerFrameLimiter,
};
use prepare::{prepare_assets, RenderOctreeNode};
use resources::ExtractedOctreeNodes;
use slab::Slab;
use std::marker::PhantomData;

pub trait OctreeNodeExtraction: Send + Sync + TypePath {
    /// ECS [`ReadOnlyQueryData`] to fetch the components to extract.
    type QueryData: ReadOnlyQueryData;

    /// Filters the entities with additional constraints.
    type QueryFilter: QueryFilter;

    type NodeData: NodeData;

    type Component: Component + ExtractComponent;

    type ExtractedNodeData: Send + Sync;

    /// Defines how the component is transferred into the "render world".
    fn extract_octree_node(
        node: &OctreeNode<Self::NodeData>,
        item: &QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::ExtractedNodeData>;
}

/// This plugin extracts visible octree nodes from the "app world" into the "render world"
/// and prepares them for the GPU. They can be accessed from the [`RenderVisibleOctreeNodes`] resource.
///
/// The `T` generic parameter refers to the type of octree node we are talking about.
/// Because an asset is an octree, we need a component to reference it.
/// This is the role of `C` generic parameter, which is the component used to find the referring octree asset.
/// So, the `C` generic parameter has to implement `Into<AssetId<Octree<T>>`.
///
/// The `A` generic parameter represents the octree node viewed by the gpu.
/// It has to implement the [`RenderOctreeNode`] trait to determine how octree nodes are converted in gpu format.
///
/// The `AFTER` generic parameter can be used to specify that `A::prepare_octree_node` should not be run until
/// `prepare_assets::<AFTER>` has completed. This allows the `prepare_octree_node` function to depend on another
/// prepared [`RenderOctreeNode`].
pub struct ExtractVisibleOctreeNodesPlugin<E, A, AFTER = ()>(PhantomData<fn() -> (E, A, AFTER)>)
where
    E: OctreeNodeExtraction,
    A: RenderOctreeNode<ExtractedOctreeNode = E::ExtractedNodeData, SourceOctreeNode = E::NodeData>,
    AFTER: RenderOctreeDependency + 'static;

impl<E, A, AFTER> Default for ExtractVisibleOctreeNodesPlugin<E, A, AFTER>
where
    E: OctreeNodeExtraction,
    for<'a> &'a E::Component: Into<AssetId<Octree<E::NodeData>>>,
    A: RenderOctreeNode<ExtractedOctreeNode = E::ExtractedNodeData, SourceOctreeNode = E::NodeData>,
    AFTER: RenderOctreeDependency + 'static,
{
    fn default() -> Self {
        ExtractVisibleOctreeNodesPlugin(PhantomData)
    }
}

impl<E, A, AFTER> Plugin for ExtractVisibleOctreeNodesPlugin<E, A, AFTER>
where
    E: OctreeNodeExtraction,
    for<'a> &'a E::Component: Into<AssetId<Octree<E::NodeData>>>,
    A: RenderOctreeNode<SourceOctreeNode = E::NodeData, ExtractedOctreeNode = E::ExtractedNodeData>,
    AFTER: RenderOctreeDependency + 'static,
{
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<E::Component>::default());
        app.init_resource::<RenderOctreeNodesBytesPerFrame>();

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.init_resource::<RenderOctreeNodesBytesPerFrameLimiter>();
        render_app
            .add_systems(ExtractSchedule, extract_render_asset_bytes_per_frame)
            .add_systems(
                Render,
                reset_render_asset_bytes_per_frame.in_set(RenderSystems::Cleanup),
            );

        render_app
            .init_resource::<ExtractedOctreeNodes<E>>()
            .init_resource::<RenderOctrees<A>>()
            .init_resource::<PrepareNextFrameOctreeNodes<A>>()
            .init_resource::<RenderOctreeIndex<E::Component>>()
            .add_systems(
                ExtractSchedule,
                (
                    extract_visible_octree_nodes::<E>.after(extract_cameras),
                    extract_render_octree_nodes::<E, A>.after(extract_visible_octree_nodes::<E>),
                ),
            );

        AFTER::register_system(
            render_app,
            prepare_assets::<E, A>.in_set(RenderSystems::PrepareAssets),
        );
    }
}

// helper to allow specifying dependencies between render assets
pub trait RenderOctreeDependency {
    fn register_system(render_app: &mut SubApp, system: ScheduleConfigs<ScheduleSystem>);
}

impl RenderOctreeDependency for () {
    fn register_system(render_app: &mut SubApp, system: ScheduleConfigs<ScheduleSystem>) {
        render_app.add_systems(Render, system);
    }
}

/// This resource stores the octree mapping to index in render world.
#[derive(Debug, Resource)]
pub struct RenderOctreeIndex<C>
where
    C: Component,
{
    pub octrees_slab: Slab<Entity>,
    pub octrees_index: HashMap<Entity, usize>,
    _phantom_data: PhantomData<C>,
}

impl<C: Component> FromWorld for RenderOctreeIndex<C> {
    fn from_world(_: &mut World) -> Self {
        RenderOctreeIndex {
            octrees_slab: Slab::new(),
            octrees_index: HashMap::new(),
            _phantom_data: PhantomData,
        }
    }
}

impl<C: Component> RenderOctreeIndex<C> {
    /// Add octree entity to index, if it already exists, does nothing.
    pub fn add_octree(&mut self, entity: Entity) -> usize {
        // TODO cleanup old octrees from the slab (if removed ?)
        *self
            .octrees_index
            .entry(entity)
            .or_insert_with(|| self.octrees_slab.insert(entity))
    }

    /// Removes an entity from the index.
    pub fn remove_octree(&mut self, entity: Entity) -> Option<usize> {
        if let Some(index) = self.octrees_index.remove(&entity) {
            self.octrees_slab.remove(index);
            Some(index)
        } else {
            None
        }
    }

    pub fn get_octree_index(&self, entity: Entity) -> Option<usize> {
        self.octrees_index.get(&entity).copied()
    }
}

/// This system extracts computed visible octree nodes and add them in the render world, for each view (camera)
pub fn extract_visible_octree_nodes<E: OctreeNodeExtraction>(
    mut commands: Commands,
    query: Extract<
        Query<(RenderEntity, &OctreesVisibility<E::NodeData, E::Component>), With<Camera>>,
    >,
    mapper: Extract<Query<&RenderEntity>>,
    mut render_octree_index: ResMut<RenderOctreeIndex<E::Component>>,
) {
    for (render_entity, visible_point_cloud_octree_3d_nodes) in query.iter() {
        let render_visible_point_cloud_octree_3d_nodes =
            RenderVisibleOctreeNodes::<E::NodeData, E::Component> {
                octrees: visible_point_cloud_octree_3d_nodes
                    .octrees
                    .clone()
                    .into_iter()
                    // for each visible octree, extract visible nodes, and store them using the render entity reference
                    .filter_map(|(entity, data)| {
                        let Ok(render_entity) = mapper.get(entity) else {
                            warn!("Render entity for PointCloudOctree3d not found");
                            return None;
                        };

                        // makes sure an index exists for this entity
                        render_octree_index.add_octree(render_entity.id());

                        Some((render_entity.id(), data))
                    })
                    .collect(),
                _phantom_data: PhantomData,
            };
        commands
            .entity(render_entity)
            .insert(render_visible_point_cloud_octree_3d_nodes);
    }
}

/// This component stores the visible nodes for each octree at view level (camera) in "render world".
#[derive(Clone, Component, Default, Debug)]
pub struct RenderVisibleOctreeNodes<T, C>
where
    T: NodeData,
    C: Component,
{
    /// The `Entity` used here refers to the "render world"
    pub octrees: HashMap<Entity, (AssetId<Octree<T>>, Vec<VisibleOctreeNode>)>,
    _phantom_data: PhantomData<C>,
}

// impl<H, T, C> ExtractComponent for OctreesVisibility<H, T, C>
// where
//     H: HierarchyNodeData,
//     T: NodeData,
//     C: Component,
// {
//     type QueryData = &'static Self;
//     type QueryFilter = With<Camera>;
//     type Out = RenderVisibleOctreeNodes<H, T, C>;
//
//     fn extract_component(
//         visible_octree_nodes: QueryItem<'_, '_, Self::QueryData>,
//     ) -> Option<Self::Out> {
//         Some(RenderVisibleOctreeNodes::<H, T, C> {
//             octrees: visible_octree_nodes.octrees.clone(),
//             _phantom_data: PhantomData,
//         })
//     }
// }

// fn iter_one_bits(mask: u8) -> impl Iterator<Item = usize> {
//     (0..8).filter(move |&i| (mask & (1 << i)) != 0)
// }
//
// #[cfg(test)]
// mod tests {
//     // Note this useful idiom: importing names from outer (for mod tests) scope.
//     use super::*;
//
//     #[test]
//     fn test_iter_one_bits() {
//         let mask: u8 = 0b01010101;
//         let indexes = iter_one_bits(mask).collect::<Vec<_>>();
//         assert_eq!(indexes, vec![0, 2, 4, 6]);
//     }
// }

/// Describes how an octree node gets extracted and prepared for rendering.
pub trait ExtractOctreeNode: NodeData + Sized + TypePath {
    /// ECS [`ReadOnlyQueryData`] to fetch the components to extract.
    type QueryData: ReadOnlyQueryData;
    /// Filters the entities with additional constraints.
    type QueryFilter: QueryFilter;

    /// The output from extraction.
    ///
    /// Returning `None` based on the queried item will remove the component from the entity in
    /// the render world. This can be used, for example, to conditionally extract octree nodes
    /// in order to disable a rendering feature on the basis of those settings, without removing
    /// the component from the entity in the main world.
    ///
    /// The output may be different from the queried component.
    /// This can be useful for example if only a subset of the fields are useful
    /// in the render world.
    ///
    /// `Out` has a [`Bundle`] trait bound instead of a [`Component`] trait bound in order to allow use cases
    /// such as tuples of components as output.
    type Out: Send + Sync;

    type Hierarchy: HierarchyNodeData;

    /// Defines how the component is transferred into the "render world".
    fn extract_octree_node(
        node: &OctreeNode<Self>,
        item: &QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out>;
}

/// This system extract visible octree nodes using the provided trait implementation for `T`: [`ExtractOctreeNode`].
/// It extracts only visible octree nodes previously computed.
pub fn extract_render_octree_nodes<E: OctreeNodeExtraction, A>(
    views: Extract<Query<(Entity, &OctreesVisibility<E::NodeData, E::Component>), With<Camera>>>,
    query: Extract<Query<(&ViewVisibility, &E::Component, E::QueryData), E::QueryFilter>>,
    octrees: Extract<Res<Assets<Octree<E::NodeData>>>>,
    mut render_octrees: ResMut<RenderOctrees<A>>,
    mut render_octree_nodes: ResMut<ExtractedOctreeNodes<E>>,
) where
    for<'a> &'a E::Component: Into<AssetId<Octree<E::NodeData>>>,
    A: RenderOctreeNode<ExtractedOctreeNode = E::ExtractedNodeData, SourceOctreeNode = E::NodeData>,
{
    // clear previously computed data
    render_octree_nodes.clear_all();

    // iter views to get visible nodes
    for (_view_entity, visible_octree_nodes) in views.iter() {
        // for each visible octree
        for (main_entity, (_, visible_octree_nodes)) in &visible_octree_nodes.octrees {
            let Ok((visibility, octree_component, item)) = query.get(*main_entity) else {
                warn!(
                    "Query item not found when extracting octree nodes: {}",
                    main_entity
                );
                continue;
            };

            if !visibility.get() {
                // octree node is not visible, skip it
                continue;
            }

            // get the octree asset to access its nodes
            let Some(octree) = octrees.get(octree_component) else {
                warn!(
                    "Octree asset {:?} not found when extracting octree nodes",
                    Into::<AssetId<Octree<E::NodeData>>>::into(octree_component)
                );
                continue;
            };

            // get the corresponding render octree (or create it) - might have been created for a previous view
            let prepared_octree = render_octrees.get_or_insert_mut(octree_component);

            let render_octree = render_octree_nodes.get_or_create_mut(octree_component);

            let removed_nodes = Vec::new();
            let mut modified_nodes = Vec::new();
            let mut added_nodes = Vec::new();

            // for each visible node
            // TODO do not iterate all nodes, but just nodes just loaded or modified (visible children)
            for VisibleOctreeNode { id: node_id, .. } in visible_octree_nodes {
                // check if the node is already prepared
                if prepared_octree.nodes.contains_key(node_id) {
                    continue;
                }

                // get it from the asset
                let Some(octree_node) = octree.node(*node_id) else {
                    warn!(
                        "Octree node {:?} not found in asset {:?}",
                        node_id,
                        Into::<AssetId<Octree<E::NodeData>>>::into(octree_component)
                    );
                    continue;
                };

                // check if it exists in render world, update octree node's metadata
                match render_octree.entry(*node_id) {
                    Entry::<_, _>::Occupied(mut entry) => {
                        let node = entry.get_mut();
                        // only the children can change
                        if !node.children.eq(&octree_node.hierarchy.children) {
                            node.children = octree_node.hierarchy.children.clone();
                            node.children_mask = octree_node.hierarchy.children_mask.clone();
                            modified_nodes.push(*node_id);
                        }
                    }
                    Entry::<_, _>::Vacant(entry) => {
                        if let Some(data) = E::extract_octree_node(octree_node, &item) {
                            added_nodes.push(*node_id);
                            entry.insert(RenderOctreeNodeData::<E::ExtractedNodeData> {
                                id: octree_node.hierarchy.id,
                                parent_id: octree_node.hierarchy.parent_id,
                                child_index: octree_node.hierarchy.child_index,
                                children: octree_node.hierarchy.children.clone(),
                                children_mask: octree_node.hierarchy.children_mask.clone(),
                                depth: octree_node.hierarchy.depth,
                                bounding_box: octree_node.hierarchy.bounding_box.clone(),
                                data,
                            });
                        }
                    }
                };
            }

            render_octree_nodes
                .added_nodes
                .insert(octree_component.into(), added_nodes);
            render_octree_nodes
                .modified_nodes
                .insert(octree_component.into(), modified_nodes);
            render_octree_nodes
                .removed_nodes
                .insert(octree_component.into(), removed_nodes);
        }
    }
}
