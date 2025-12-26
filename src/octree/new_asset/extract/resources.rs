use super::prepare::RenderOctreeNode;
use super::render_asset::{RenderOctree, RenderOctreeNodeData};
use super::OctreeNodeExtraction;
use crate::octree::new_asset::asset::NewOctree;
use crate::octree::storage::NodeId;
use bevy_asset::AssetId;
use bevy_ecs::prelude::*;
use bevy_platform::collections::{HashMap, HashSet};

/// All assets that should be prepared next frame.
#[derive(Resource)]
pub struct PrepareNextFrameOctreeNodes<A: RenderOctreeNode> {
    pub(crate) assets: Vec<(
        AssetId<NewOctree<A::SourceOctreeNode>>,
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
    HashMap<AssetId<NewOctree<A::SourceOctreeNode>>, RenderOctree<A>>,
);

impl<A: RenderOctreeNode> Default for RenderOctrees<A> {
    fn default() -> Self {
        Self(HashMap::new())
    }
}

impl<A: RenderOctreeNode> RenderOctrees<A> {
    pub fn get(
        &self,
        id: impl Into<AssetId<NewOctree<A::SourceOctreeNode>>>,
    ) -> Option<&RenderOctree<A>> {
        self.0.get(&id.into())
    }

    pub fn get_or_insert_mut(
        &mut self,
        id: impl Into<AssetId<NewOctree<A::SourceOctreeNode>>>,
    ) -> &mut RenderOctree<A> {
        self.0.entry(id.into()).or_default()
    }

    pub fn remove(
        &mut self,
        id: impl Into<AssetId<NewOctree<A::SourceOctreeNode>>>,
    ) -> Option<RenderOctree<A>> {
        self.0.remove(&id.into())
    }
}

/// Contains all extracted octree nodes for preparing
#[derive(Resource)]
pub struct ExtractedOctreeNodes<E: OctreeNodeExtraction> {
    pub octrees: HashMap<
        AssetId<NewOctree<E::NodeData>>,
        HashMap<NodeId, RenderOctreeNodeData<E::ExtractedNodeData>>,
    >,

    /// contains all already prepared octree nodes living in render world
    pub prepared_octrees: HashMap<AssetId<NewOctree<E::NodeData>>, HashSet<NodeId>>,

    /// IDs of the assets that were removed this frame.
    ///
    /// These assets will not be present in [`ExtractedAssets::extracted`].
    // removed_assets: HashSet<AssetId<NewOctree<E::NodeHierarchy, E::NodeData>>>,
    pub removed_nodes: HashMap<AssetId<NewOctree<E::NodeData>>, Vec<NodeId>>,

    /// IDs of the assets that were modified this frame.
    // modified_assets: HashSet<AssetId<NewOctree<E::NodeHierarchy, E::NodeData>>>,
    pub modified_nodes: HashMap<AssetId<NewOctree<E::NodeData>>, Vec<NodeId>>,

    /// IDs of the assets that were added this frame.
    // added_assets: HashSet<AssetId<NewOctree<E::NodeHierarchy, E::NodeData>>>,
    pub added_nodes: HashMap<AssetId<NewOctree<E::NodeData>>, Vec<NodeId>>,
}

impl<E: OctreeNodeExtraction> Default for ExtractedOctreeNodes<E> {
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

impl<E: OctreeNodeExtraction> ExtractedOctreeNodes<E> {
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
        id: impl Into<AssetId<NewOctree<E::NodeData>>>,
    ) -> &mut HashMap<NodeId, RenderOctreeNodeData<E::ExtractedNodeData>> {
        self.octrees
            .entry(id.into())
            .or_insert_with(Default::default)
    }
}
