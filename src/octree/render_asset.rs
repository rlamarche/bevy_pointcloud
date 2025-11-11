use super::asset::OctreeNode;
pub use super::storage::NodeId;
use bevy_platform::collections::HashMap;
use std::fmt::Debug;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum InsertNodeError {
    #[error("parent does not exists")]
    ParentNotExists,
    #[error("parent has already 8 children")]
    ParentChildrenFull,
}

pub struct RenderOctree<A>
where
    A: Send + Sync,
{
    pub(crate) nodes: HashMap<NodeId, OctreeNode<A>>,
    pub(crate) root_id: Option<NodeId>,
}

impl<A: Send + Sync> Default for RenderOctree<A> {
    fn default() -> Self {
        Self {
            nodes: Default::default(),
            root_id: Default::default(),
        }
    }
}

impl<A> RenderOctree<A>
where
    A: Send + Sync,
{
    pub fn insert(&mut self, node_id: NodeId, node: OctreeNode<A>) {
        self.nodes.insert(node_id, node);
    }

    pub fn remove(&mut self, node_id: NodeId, node: OctreeNode<A>) -> Option<OctreeNode<A>> {
        self.nodes.remove(&node_id)
    }
}
