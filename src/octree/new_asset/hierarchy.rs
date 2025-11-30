use bevy_camera::primitives::Aabb;
use bevy_reflect::TypePath;
use crate::octree::storage::NodeId;

#[derive(Debug, Clone, Copy)]
pub enum HierarchyNodeStatus {
    Loaded,
    Proxy,
    Loading,
}

pub trait HierarchyNodeData: Send + Sync + TypePath + Clone {

}



/// This type contains the hierarchy only data of an octree node
/// It can be in state where it's loaded or not (`status`)
#[derive(Debug, Clone)]
pub struct HierarchyOctreeNode<H>
where
    H: HierarchyNodeData,
{
    pub id: NodeId,
    pub status: HierarchyNodeStatus,
    pub child_index: usize,
    pub parent_id: Option<NodeId>,
    pub children: [NodeId; 8],
    pub children_mask: u8,
    pub bounding_box: Aabb,
    pub depth: u32,
    pub data: H,
}

/// This type contains the hierarchy only data of an octree node
/// It can be in state where it's loaded or not (`status`)
#[derive(Debug, Clone)]
pub struct HierarchyNode<H>
where
    H: HierarchyNodeData,
{
    pub status: HierarchyNodeStatus,
    pub child_index: usize,
    pub parent_id: Option<usize>,
    pub bounding_box: Aabb,
    pub data: H,
}
