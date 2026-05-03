use std::marker::PhantomData;

use super::asset::{RenderOctree, RenderOctreeNodeData};
use super::node::RenderOctreeNode;
use crate::octree::asset::Octree;
use crate::octree::extract::OctreeNodeExtraction;
use crate::octree::extract::render::asset::RenderOctreeNodeAllocation;
use crate::octree::extract::resources::NodeAllocation;
use crate::octree::storage::NodeId;
use bevy_asset::AssetId;
use bevy_ecs::prelude::*;
use bevy_platform::collections::HashMap;
use slab::Slab;

/// This resource stores the octree mapping to index in render world.
#[derive(Debug, Resource)]
pub struct RenderOctreeIndex<C>
where
    C: Component,
{
    pub(crate) octrees_slab: Slab<Entity>,
    pub(crate) octrees_index: HashMap<Entity, usize>,
    pub(crate) added_octrees: Vec<Entity>,
    pub(crate) removed_octrees: Vec<Entity>,
    pub(crate) _phantom_data: PhantomData<C>,
}

impl<C: Component> FromWorld for RenderOctreeIndex<C> {
    fn from_world(_: &mut World) -> Self {
        RenderOctreeIndex {
            octrees_slab: Slab::new(),
            octrees_index: HashMap::new(),
            added_octrees: Vec::new(),
            removed_octrees: Vec::new(),
            _phantom_data: PhantomData,
        }
    }
}

impl<C: Component> RenderOctreeIndex<C> {
    /// Add octree entity to index, if it already exists, does nothing.
    pub fn add_octree(&mut self, entity: Entity) -> usize {
        let octree_index = *self
            .octrees_index
            .entry(entity)
            .or_insert_with(|| self.octrees_slab.insert(entity));

        self.added_octrees.push(entity);

        octree_index
    }

    /// Removes an entity from the index.
    /// TODO: call this function
    pub fn remove_octree(&mut self, entity: Entity) -> Option<usize> {
        if let Some(index) = self.octrees_index.remove(&entity) {
            self.octrees_slab.remove(index);
            self.removed_octrees.push(entity);
            Some(index)
        } else {
            None
        }
    }

    pub fn get_octree_index(&self, entity: Entity) -> Option<usize> {
        self.octrees_index.get(&entity).copied()
    }
}

/// All assets that should be prepared next frame.
#[derive(Resource)]
pub struct PrepareNextFrameOctreeNodes<A: RenderOctreeNode> {
    pub(crate) assets: Vec<(
        AssetId<Octree<A::SourceOctreeNode>>,
        RenderOctreeNodeData<A::ExtractedOctreeNode>,
    )>,
}

impl<A: RenderOctreeNode> Default for PrepareNextFrameOctreeNodes<A> {
    fn default() -> Self {
        Self {
            assets: Default::default(),
        }
    }
}

/// Stores all GPU representations ([`RenderAsset`])
/// of [`RenderAsset::SourceAsset`] as long as they exist.
#[derive(Resource)]
pub struct RenderOctrees<A: RenderOctreeNode>(
    HashMap<AssetId<Octree<A::SourceOctreeNode>>, RenderOctree<A>>,
);

impl<A: RenderOctreeNode> Default for RenderOctrees<A> {
    fn default() -> Self {
        Self(HashMap::new())
    }
}

impl<A: RenderOctreeNode> RenderOctrees<A> {
    pub fn get(
        &self,
        id: impl Into<AssetId<Octree<A::SourceOctreeNode>>>,
    ) -> Option<&RenderOctree<A>> {
        self.0.get(&id.into())
    }

    pub fn get_or_insert_mut(
        &mut self,
        id: impl Into<AssetId<Octree<A::SourceOctreeNode>>>,
    ) -> &mut RenderOctree<A> {
        self.0.entry(id.into()).or_default()
    }

    pub fn remove(
        &mut self,
        id: impl Into<AssetId<Octree<A::SourceOctreeNode>>>,
    ) -> Option<RenderOctree<A>> {
        self.0.remove(&id.into())
    }
}

/// Contains all extracted octree nodes for preparing
#[derive(Resource)]
pub struct ExtractedOctreeNodes<E: OctreeNodeExtraction> {
    pub(crate) max_instances: u32,
    pub(crate) octrees: HashMap<
        AssetId<Octree<E::NodeData>>,
        HashMap<NodeId, RenderOctreeNodeData<E::ExtractedNodeData>>,
    >,

    /// IDs of the assets that were removed this frame.
    ///
    /// These assets will not be present in [`ExtractedAssets::extracted`].
    // removed_assets: HashSet<AssetId<Octree<E::NodeHierarchy, E::NodeData>>>,
    pub(crate) removed_nodes: HashMap<AssetId<Octree<E::NodeData>>, Vec<NodeId>>,

    /// IDs of the assets that were modified this frame.
    // modified_assets: HashSet<AssetId<Octree<E::NodeHierarchy, E::NodeData>>>,
    pub(crate) modified_nodes: HashMap<AssetId<Octree<E::NodeData>>, Vec<NodeId>>,

    /// IDs of the assets that were added this frame.
    // added_assets: HashSet<AssetId<Octree<E::NodeHierarchy, E::NodeData>>>,
    pub(crate) added_nodes: HashMap<AssetId<Octree<E::NodeData>>, Vec<NodeId>>,

    pub(crate) freed_nodes_this_frame: Vec<NodeAllocation<E::NodeData>>,
    pub(crate) allocated_nodes_this_frame: Vec<NodeAllocation<E::NodeData>>,
}

impl<E: OctreeNodeExtraction> Default for ExtractedOctreeNodes<E> {
    fn default() -> Self {
        Self {
            max_instances: 0,
            octrees: HashMap::new(),
            // removed_assets: Default::default(),
            removed_nodes: Default::default(),
            // modified_assets: Default::default(),
            modified_nodes: Default::default(),
            // added_assets: Default::default(),
            added_nodes: Default::default(),
            freed_nodes_this_frame: Default::default(),
            allocated_nodes_this_frame: Default::default(),
        }
    }
}

impl<E: OctreeNodeExtraction> ExtractedOctreeNodes<E> {
    pub fn clear_all(&mut self) {
        // self.added_assets.clear();
        self.added_nodes.clear();
        // self.modified_assets.clear();
        self.modified_nodes.clear();
        // self.removed_assets.clear();
        self.removed_nodes.clear();

        self.allocated_nodes_this_frame.clear();
        self.freed_nodes_this_frame.clear();
    }

    pub fn get_or_create_mut(
        &mut self,
        id: impl Into<AssetId<Octree<E::NodeData>>>,
    ) -> &mut HashMap<NodeId, RenderOctreeNodeData<E::ExtractedNodeData>> {
        self.octrees
            .entry(id.into())
            .or_insert_with(Default::default)
    }
}

/// Contains all allocated octree nodes ready for render
#[derive(Resource)]
pub struct AllocatedOctreeNodes<E: OctreeNodeExtraction> {
    pub(crate) allocations:
        HashMap<AssetId<Octree<E::NodeData>>, HashMap<NodeId, RenderOctreeNodeAllocation>>,
}

impl<E: OctreeNodeExtraction> Default for AllocatedOctreeNodes<E> {
    fn default() -> Self {
        Self {
            allocations: HashMap::new(),
        }
    }
}

impl<E: OctreeNodeExtraction> AllocatedOctreeNodes<E> {
    pub fn get_or_create_mut(
        &mut self,
        id: impl Into<AssetId<Octree<E::NodeData>>>,
    ) -> &mut HashMap<NodeId, RenderOctreeNodeAllocation> {
        self.allocations
            .entry(id.into())
            .or_insert_with(Default::default)
    }
}
