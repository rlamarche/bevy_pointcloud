use crate::octree::new_asset::hierarchy::{HierarchyNodeData, HierarchyOctreeNode};
use bevy_reflect::TypePath;

pub trait NodeData: Send + Sync + TypePath {}

impl<T: Send + Sync + TypePath> NodeData for T {}

#[derive(Debug, Clone, Copy)]
pub enum NodeStatus {
    HierarchyOnly,
    Loading,
    Loaded,
}

/// This type contains the hierarchy only data of an octree node
/// It can be in state where it's loaded or not (`status`)
#[derive(Clone, Debug)]
pub struct OctreeNode<H, T>
where
    H: HierarchyNodeData,
    T: NodeData,
{
    pub hierarchy: HierarchyOctreeNode<H>,
    pub status: NodeStatus,
    pub data: Option<T>,
}
