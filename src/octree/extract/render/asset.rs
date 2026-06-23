use std::fmt::Debug;

use bevy_camera::primitives::Aabb;
use bevy_platform::collections::HashMap;
use thiserror::Error;

use crate::octree::storage::NodeId;

#[derive(Error, Debug)]
pub enum InsertNodeError {
    #[error("parent does not exists")]
    ParentNotExists,
    #[error("parent has already 8 children")]
    ParentChildrenFull,
}

pub struct RenderOctree<ERA> {
    pub(crate) nodes: HashMap<NodeId, RenderOctreeNodeData<ERA>>,
    #[allow(unused)]
    pub(crate) root_id: Option<NodeId>,
}

impl<ERA> Default for RenderOctree<ERA> {
    fn default() -> Self {
        Self {
            nodes: Default::default(),
            root_id: Default::default(),
        }
    }
}

impl<ERA> RenderOctree<ERA> {
    pub fn insert(&mut self, node_id: NodeId, node: RenderOctreeNodeData<ERA>) {
        self.nodes.insert(node_id, node);
    }

    pub fn remove(&mut self, node_id: NodeId) -> Option<RenderOctreeNodeData<ERA>> {
        self.nodes.remove(&node_id)
    }
}

#[derive(Clone, Debug)]
pub struct RenderOctreeNodeAllocation {
    /// offset in bytes
    pub offset: u64,
    /// size in bytes
    pub size: u64,
    /// offset in instance count
    pub start: u32,
    /// number of instances
    pub count: u32,
}

#[derive(Clone, Debug)]
pub struct RenderOctreeNodeData<T> {
    pub id: NodeId,
    pub child_index: u8,
    pub parent_id: Option<NodeId>,
    pub children: [NodeId; 8],
    pub children_mask: u8,
    pub bounding_box: Aabb,
    pub depth: u32,
    pub data: T,
    pub allocation: RenderOctreeNodeAllocation,
}
