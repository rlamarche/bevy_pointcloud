use crate::octree::{loader::LoadedHierarchyNode, storage::NodeId};
use bevy_camera::primitives::Aabb;
use bevy_reflect::TypePath;
use std::{any::Any, sync::Arc};

#[derive(Debug, Clone, Copy)]
pub enum HierarchyNodeStatus {
    Proxy,
    Loading,
    Loaded,
}

pub trait HierarchyNodeData: Send + Sync + TypePath + Clone {}

impl<T: Send + Sync + TypePath + Clone> HierarchyNodeData for T {}

/// This type contains the hierarchy only data of an octree node
/// It can be in state where it's loaded or not (`status`)
#[derive(Clone, Debug)]
pub struct HierarchyNode {
    pub status: HierarchyNodeStatus,
    pub child_index: u8,
    pub parent_id: Option<usize>,
    pub bounding_box: Aabb,
    pub data: Arc<dyn Any + Send + Sync>,
}

#[derive(Clone, Debug)]
pub struct HierarchyOctreeNode {
    pub id: NodeId,
    // e.g. r, r0, r3, r4, r01, r07, r30, ...
    pub name: Arc<str>,
    pub status: HierarchyNodeStatus,
    pub child_index: u8,
    pub parent_id: Option<NodeId>,
    pub children: [NodeId; 8],
    pub children_mask: u8,
    pub bounding_box: Aabb,
    pub depth: u32,
    pub data: Arc<dyn Any + Send + Sync>,
}

impl<H: HierarchyNodeData> From<LoadedHierarchyNode<H>> for HierarchyNode {
    fn from(value: LoadedHierarchyNode<H>) -> Self {
        Self {
            status: value.status,
            child_index: value.child_index,
            parent_id: value.parent_id,
            bounding_box: value.bounding_box,
            data: Arc::new(value),
        }
    }
}
