use crate::octree::asset::{NodeId, Octree, OctreeNode};
use crate::octree::visibility::prepare::{RenderOctreeNode, RenderOctrees};
use crate::octree::visibility::{VisibleOctreeNode, VisibleOctreeNodes};
use bevy_asset::{AssetId, Assets};
use bevy_camera::visibility::ViewVisibility;
use bevy_camera::Camera;
use bevy_ecs::prelude::*;
use bevy_ecs::query::{QueryFilter, QueryItem, ReadOnlyQueryData};
use bevy_log::prelude::*;
use bevy_platform::collections::hash_map::Entry;
use bevy_platform::collections::{HashMap, HashSet};
use bevy_reflect::TypePath;
use bevy_render::Extract;

/// Describes how an octree node gets extracted and prepared for rendering.
pub trait ExtractOctreeNode: Send + Sync + Sized + TypePath {
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

    /// Defines how the component is transferred into the "render world".
    fn extract_octree_node(
        node: &OctreeNode<Self>,
        item: &QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out>;
}

/// This system extract visible octree nodes using the provided trait implementation for `T`: [`ExtractOctreeNode`].
/// It extracts only visible octree nodes previously computed.
pub fn extract_render_octree_nodes<T, C, A>(
    views: Extract<Query<(Entity, &VisibleOctreeNodes<T>), With<Camera>>>,
    query: Extract<Query<(&ViewVisibility, &C, T::QueryData), T::QueryFilter>>,
    octrees: Extract<Res<Assets<Octree<T>>>>,
    mut render_octrees: ResMut<RenderOctrees<A>>,
    mut render_octree_nodes: ResMut<ExtractedOctreeNodes<T>>,
) where
    T: ExtractOctreeNode,
    C: Component,
    for<'a> &'a C: Into<AssetId<Octree<T>>>,
    A: RenderOctreeNode<ExtractedOctreeNode = T::Out, SourceOctreeNode = T>,
{
    // clear previously computed data
    render_octree_nodes.clear_all();

    // iter views to get visible nodes
    for (_view_entity, visible_octree_nodes) in views.iter() {
        // for each visible octree
        for (main_entity, visible_octree_nodes) in &visible_octree_nodes.octrees {
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
                    Into::<AssetId<Octree<T>>>::into(octree_component)
                );
                continue;
            };

            // get the corresponding render octree (or create it) - might have been created for a previous view

            // TODO find a way to prevent duplicating (using another resource ?)
            let prepared_octree = render_octrees.get_or_insert_mut(octree_component);
            // let prepared_nodes = render_octree_nodes
            //     .prepared_octrees
            //     .get(&octree_component.into())
            //     .map(Clone::clone)
            //     .unwrap_or_else(|| HashSet::new());

            let render_octree = render_octree_nodes.get_or_create_mut(octree_component);

            let removed_nodes = Vec::new();
            let mut modified_nodes = Vec::new();
            let mut added_nodes = Vec::new();

            // for each visible node
            for VisibleOctreeNode { id: node_id, .. } in visible_octree_nodes {
                // check if the node is already prepared
                if prepared_octree.nodes.contains_key(node_id) {
                    continue;
                }

                // get it from the asset
                let Some(octree_node) = octree.get(*node_id) else {
                    warn!(
                        "Octree node {:?} not found in asset {:?}",
                        node_id,
                        Into::<AssetId<Octree<T>>>::into(octree_component)
                    );
                    continue;
                };

                // check if it exists in render world, update octree node's metadata
                match render_octree.entry(*node_id) {
                    Entry::<_, _>::Occupied(mut entry) => {
                        let node = entry.get_mut();
                        // only the children can change
                        if !node.children.eq(&octree_node.children) {
                            node.children = octree_node.children.clone();
                            node.children_mask = octree_node.children_mask.clone();
                            modified_nodes.push(*node_id);
                        }
                    }
                    Entry::<_, _>::Vacant(entry) => {
                        if let Some(data) = T::extract_octree_node(octree_node, &item) {
                            added_nodes.push(*node_id);
                            entry.insert(OctreeNode {
                                id: octree_node.id,
                                parent_id: octree_node.parent_id,
                                child_index: octree_node.child_index,
                                children: octree_node.children.clone(),
                                children_mask: octree_node.children_mask.clone(),
                                bounding_box: octree_node.bounding_box.clone(),
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

/// Contains all extracted octree nodes for preparing
#[derive(Resource)]
pub struct ExtractedOctreeNodes<T: ExtractOctreeNode> {
    pub octrees: HashMap<AssetId<Octree<T>>, HashMap<NodeId, OctreeNode<T::Out>>>,

    /// contains all already prepared octree nodes living in render world
    pub prepared_octrees: HashMap<AssetId<Octree<T>>, HashSet<NodeId>>,

    /// IDs of the assets that were removed this frame.
    ///
    /// These assets will not be present in [`ExtractedAssets::extracted`].
    // removed_assets: HashSet<AssetId<Octree<T>>>,
    pub removed_nodes: HashMap<AssetId<Octree<T>>, Vec<NodeId>>,

    /// IDs of the assets that were modified this frame.
    // modified_assets: HashSet<AssetId<Octree<T>>>,
    pub modified_nodes: HashMap<AssetId<Octree<T>>, Vec<NodeId>>,

    /// IDs of the assets that were added this frame.
    // added_assets: HashSet<AssetId<Octree<T>>>,
    pub added_nodes: HashMap<AssetId<Octree<T>>, Vec<NodeId>>,
}

impl<T: ExtractOctreeNode> Default for ExtractedOctreeNodes<T> {
    fn default() -> Self {
        Self {
            octrees: HashMap::new(),
            prepared_octrees: HashMap::new(),
            // removed_assets: Default::default(),
            removed_nodes: Default::default(),
            // modified_assets: Default::default(),
            modified_nodes: Default::default(),
            // added_assets: Default::default(),
            added_nodes: Default::default(),
        }
    }
}

impl<T: ExtractOctreeNode> ExtractedOctreeNodes<T> {
    pub fn clear_all(&mut self) {
        // self.added_assets.clear();
        self.added_nodes.clear();
        // self.modified_assets.clear();
        self.modified_nodes.clear();
        // self.removed_assets.clear();
        self.removed_nodes.clear();
    }

    pub fn get_or_create_mut(
        &mut self,
        id: impl Into<AssetId<Octree<T>>>,
    ) -> &mut HashMap<NodeId, OctreeNode<T::Out>> {
        self.octrees
            .entry(id.into())
            .or_insert_with(Default::default)
    }
}
