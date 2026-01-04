use crate::octree::hierarchy::{HierarchyNode, HierarchyNodeData, HierarchyNodeStatus, HierarchyOctreeNode};
use crate::octree::node::NodeData;
use async_trait::async_trait;
use bevy_ecs::error::BevyError;
use bevy_ecs::prelude::*;
use bevy_reflect::TypePath;
use std::fmt::Display;
use bevy_camera::primitives::Aabb;

#[derive(Debug, Clone)]
pub struct LoadedHierarchyNode<H>
where
    H: HierarchyNodeData,
{
    pub status: HierarchyNodeStatus,
    pub child_index: u8,
    pub parent_id: Option<usize>,
    pub bounding_box: Aabb,
    pub data: H,
}


#[async_trait]
pub trait OctreeLoader<T>: Send + Sync + Sized + TypePath
where
    T: NodeData,
{
    type Hierarchy: HierarchyNodeData;

    type Error: Into<BevyError> + Send + Sync + Display;

    /// Instantiate a new hierarchy from a provided url
    async fn from_url(url: &str) -> Result<Self, Self::Error>;

    /// This method must load the initial octree hierarchy in a flat structure.
    /// The return value is a vector, the first item is the root,
    /// then all children are referenced in the parent with their indice in the vec.
    /// Every child should also reference its parent through its indice too.
    async fn load_initial_hierarchy(&self) -> Result<Vec<LoadedHierarchyNode<Self::Hierarchy>>, Self::Error>;

    /// This method must load the provided node sub hierarchy.
    /// The return format is the same as described in [`OctreeLoader::load_initial_hierarchy`].
    /// So, the provided node is expected to be the first in the returned vector.
    /// The provided node **must** be in [`HierarchyNodeStatus::Proxy`] state, or an error might be thrown.
    async fn load_hierarchy(
        &self,
        node: &LoadedHierarchyNode<Self::Hierarchy>,
    ) -> Result<Vec<LoadedHierarchyNode<Self::Hierarchy>>, Self::Error>;

    async fn load_node_data(&self, node: &LoadedHierarchyNode<Self::Hierarchy>) -> Result<T, Self::Error>;
}
