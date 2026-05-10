use std::fmt::Debug;

use bevy_camera::primitives::Aabb;
use bevy_platform::collections::HashMap;
use thiserror::Error;

use super::node::RenderOctreeNode;
use crate::octree::storage::NodeId;

#[derive(Error, Debug)]
pub enum InsertNodeError {
    #[error("parent does not exists")]
    ParentNotExists,
    #[error("parent has already 8 children")]
    ParentChildrenFull,
}

pub struct RenderOctree<A>
where
    A: RenderOctreeNode,
{
    pub(crate) nodes: HashMap<NodeId, RenderOctreeNodeData<A>>,
    #[allow(unused)]
    pub(crate) root_id: Option<NodeId>,
}

impl<A: RenderOctreeNode> Default for RenderOctree<A> {
    fn default() -> Self {
        Self {
            nodes: Default::default(),
            root_id: Default::default(),
        }
    }
}

impl<A> RenderOctree<A>
where
    A: RenderOctreeNode,
{
    pub fn insert(&mut self, node_id: NodeId, node: RenderOctreeNodeData<A>) {
        self.nodes.insert(node_id, node);
    }

    pub fn remove(&mut self, node_id: NodeId) -> Option<RenderOctreeNodeData<A>> {
        self.nodes.remove(&node_id)
    }
}

#[derive(Clone, Debug)]
pub struct RenderOctreeNodeAllocation {
    pub start: u32,
    pub count: u32,
}

#[derive(Clone, Debug)]
pub struct RenderOctreeNodeData<T>
where
    T: Send + Sync,
{
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
