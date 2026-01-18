use crate::octree::asset::Octree;
use crate::octree::hierarchy::HierarchyOctreeNode;
use crate::octree::storage::NodeId;
use bevy_asset::AssetId;
use bevy_reflect::TypePath;
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};

pub trait NodeData: Send + Sync + TypePath {
    fn size(&self) -> usize;
}

#[derive(Debug, Clone, Copy)]
pub enum NodeStatus {
    HierarchyOnly,
    Loading,
    Loaded,
}

/// This type contains the hierarchy only data of an octree node
/// It can be in state where it's loaded or not (`status`)
#[derive(Clone, Debug)]
pub struct OctreeNode<T: NodeData> {
    pub hierarchy: HierarchyOctreeNode,
    pub status: NodeStatus,
    pub data: Option<T>,
}

pub struct OctreeNodeKey<T: NodeData> {
    pub octree_id: AssetId<Octree<T>>,
    pub node_id: NodeId,
}

impl<T: NodeData> Debug for OctreeNodeKey<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OctreeNodeKey")
            .field("octree_id", &self.octree_id)
            .field("node_id", &self.node_id)
            .finish()
    }
}

impl<T: NodeData> Clone for OctreeNodeKey<T> {
    fn clone(&self) -> Self {
        Self {
            octree_id: self.octree_id,
            node_id: self.node_id,
        }
    }
}

impl<T: NodeData> Hash for OctreeNodeKey<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.octree_id.hash(state);
        self.node_id.hash(state);
    }
}

impl<T: NodeData> PartialEq for OctreeNodeKey<T> {
    fn eq(&self, other: &Self) -> bool {
        self.octree_id == other.octree_id && self.node_id == other.node_id
    }
}

impl<T: NodeData> Eq for OctreeNodeKey<T> {}
