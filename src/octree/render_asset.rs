use super::asset::OctreeNode;
pub use super::storage::NodeId;
use crate::octree::storage::GenerationalSlab;
use bevy_asset::Asset;
use bevy_camera::primitives::Aabb;
use bevy_platform::collections::{HashMap, HashSet};
use bevy_reflect::TypePath;
use std::fmt::Debug;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum InsertNodeError {
    #[error("parent does not exists")]
    ParentNotExists,
    #[error("parent has already 8 children")]
    ParentChildrenFull,
}

#[derive(Debug, Clone, Asset, TypePath)]
pub struct RenderOctree<T>
where
    T: Clone + Debug + Send + Sync + TypePath,
{
    pub(crate) nodes: HashMap<NodeId, OctreeNode<T>>,
    pub(crate) root_id: Option<NodeId>,
}

impl<T> Default for RenderOctree<T>
where
    T: Clone + Debug + Send + Sync + TypePath,
{
    fn default() -> Self {
        Self {
            nodes: Default::default(),
            root_id: Default::default(),
        }
    }
}

impl<T> RenderOctree<T>
where
    T: Clone + Debug + Send + Sync + TypePath,
{
    pub fn insert(&mut self, node_id: NodeId, node: OctreeNode<T>) {
        self.nodes.insert(node_id, node);
    }

    pub fn remove(&mut self, node_id: NodeId, node: OctreeNode<T>) -> Option<OctreeNode<T>> {
        self.nodes.remove(&node_id)
    }
}
