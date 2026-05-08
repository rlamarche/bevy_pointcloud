use crate::octree::{
    hierarchy::{HierarchyNode, HierarchyNodeData, HierarchyNodeStatus, HierarchyOctreeNode},
    node::NodeData,
};
use async_trait::async_trait;
use bevy_camera::primitives::Aabb;
use bevy_ecs::{error::BevyError, prelude::*};
use std::fmt::Display;

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
pub trait OctreeLoader<T: NodeData>: Send + Sync + Sized + 'static
where
    T: NodeData,
{
    type Source: Send + Sync + 'static;
    type Hierarchy: HierarchyNodeData;

    type Error: Into<BevyError> + Send + Sync + Display;

    /// Instantiate a new hierarchy from a provided url
    async fn from_source(source: Self::Source) -> Result<Self, Self::Error>;

    /// This method must load the initial octree hierarchy in a flat structure.
    /// The return value is a vector, the first item is the root,
    /// then all children are referenced in the parent with their indice in the vec.
    /// Every child should also reference its parent through its indice too.
    async fn load_initial_hierarchy(
        &self,
    ) -> Result<Vec<LoadedHierarchyNode<Self::Hierarchy>>, Self::Error>;

    /// This method must load the provided node sub hierarchy.
    /// The return format is the same as described in [`OctreeLoader::load_initial_hierarchy`].
    /// So, the provided node is expected to be the first in the returned vector.
    /// The provided node **must** be in [`HierarchyNodeStatus::Proxy`] state, or an error might be thrown.
    async fn load_hierarchy(
        &self,
        node: &LoadedHierarchyNode<Self::Hierarchy>,
    ) -> Result<Vec<LoadedHierarchyNode<Self::Hierarchy>>, Self::Error>;

    async fn load_node_data(
        &self,
        node: &LoadedHierarchyNode<Self::Hierarchy>,
    ) -> Result<T, Self::Error>;
}

#[async_trait]
pub trait ErasedOctreeLoader<T: NodeData>: Send + Sync + 'static {
    /// This method must load the initial octree hierarchy in a flat structure.
    /// The return value is a vector, the first item is the root,
    /// then all children are referenced in the parent with their indice in the vec.
    /// Every child should also reference its parent through its indice too.
    async fn load_initial_hierarchy(&self) -> Result<Vec<HierarchyNode>, BevyError>;

    /// This method must load the provided node sub hierarchy.
    /// The return format is the same as described in [`OctreeLoader::load_initial_hierarchy`].
    /// So, the provided node is expected to be the first in the returned vector.
    /// The provided node **must** be in [`HierarchyNodeStatus::Proxy`] state, or an error might be thrown.
    async fn load_hierarchy(
        &self,
        node: &HierarchyOctreeNode,
    ) -> Result<Vec<HierarchyNode>, BevyError>;

    async fn load_node_data(&self, node: &HierarchyOctreeNode) -> Result<T, BevyError>;
}

#[async_trait]
impl<T: NodeData, L: OctreeLoader<T>> ErasedOctreeLoader<T> for L {
    async fn load_initial_hierarchy(&self) -> Result<Vec<HierarchyNode>, BevyError> {
        let initial_hierarchy = <Self as OctreeLoader<T>>::load_initial_hierarchy(self)
            .await
            .map_err(|err| err.into())?;

        Ok(initial_hierarchy
            .into_iter()
            .map(|node| HierarchyNode::from(node))
            .collect())
    }

    async fn load_hierarchy(
        &self,
        node: &HierarchyOctreeNode,
    ) -> Result<Vec<HierarchyNode>, BevyError> {
        let Ok(loaded_hierarchy) = node
            .data
            .clone()
            .downcast::<LoadedHierarchyNode<L::Hierarchy>>()
        else {
            return Err("Unable to downcast LoadedHierarchyNode".into());
        };

        let loaded_nodes = self
            .load_hierarchy(&loaded_hierarchy)
            .await
            .map_err(|err| err.into())?;

        Ok(loaded_nodes
            .into_iter()
            .map(|node| HierarchyNode::from(node))
            .collect())
    }

    async fn load_node_data(&self, node: &HierarchyOctreeNode) -> Result<T, BevyError> {
        let Ok(loaded_hierarchy) = node
            .data
            .clone()
            .downcast::<LoadedHierarchyNode<L::Hierarchy>>()
        else {
            return Err("Unable to downcast LoadedHierarchyNode".into());
        };

        self.load_node_data(&loaded_hierarchy)
            .await
            .map_err(|err| err.into())
    }
}
