use std::ops::Deref;

use bevy::{
    camera::primitives::Aabb,
    ecs::error::BevyError,
    mesh::Mesh,
    tasks::{BoxedFuture, ConditionalSendFuture},
};

use crate::{ChildIndex, HierarchyData, HierarchyNode, HierarchyNodeStatus};

pub trait PointCloudLoader: Send + Sync + Sized + 'static {
    type Hierarchy: Send + Sync + 'static;
    type Error: Into<BevyError>;

    /// This method must load the initial point cloud octree hierarchy in a flat structure.
    /// The return value is a vector, the first item is the root,
    /// then all children are referenced in the parent with their indice in the vec.
    /// Every child should also reference its parent through its indice too.
    fn load_initial_hierarchy(
        &self,
    ) -> impl ConditionalSendFuture<
        Output = Result<Vec<LoadedHierarchyNode<Self::Hierarchy>>, Self::Error>,
    >;

    /// This method must load the provided node sub hierarchy.
    /// The return format is the same as described in [`PointCloudLoader::load_initial_hierarchy`].
    /// So, the provided node is expected to be the first in the returned vector.
    /// The provided node **must** be in [`HierarchyNodeStatus::Proxy`] state, or an error might be
    /// thrown.
    #[expect(unused, reason = "Prevent suffixing parameter with _.")]
    fn load_sub_hierarchy(
        &self,
        node: &Self::Hierarchy,
    ) -> impl ConditionalSendFuture<
        Output = Result<Vec<LoadedHierarchyNode<Self::Hierarchy>>, Self::Error>,
    > {
        Box::pin(async move { Ok(vec![]) })
    }

    /// This method must load the chunk data into a gpu friendly format for rendering.
    /// This format is [`Mesh`] for the moment to reuse Bevy part, it might change in the future.
    fn load_chunk(
        &self,
        node: &Self::Hierarchy,
    ) -> impl ConditionalSendFuture<Output = Result<Mesh, Self::Error>>;
}

#[derive(Clone, Debug)]
pub struct LoadedHierarchyNode<H> {
    pub status: HierarchyNodeStatus,
    pub point_count: usize,
    pub child_index: ChildIndex,
    pub parent_id: Option<usize>,
    pub aabb: Option<Aabb>,
    pub data: H,
}

pub trait ErasedPointCloudLoader: Send + Sync + 'static {
    /// Erased version of [`PointCloudLoader::load_initial_hierarchy`]
    fn load_initial_hierarchy<'a>(
        &'a self,
    ) -> BoxedFuture<'a, Result<Vec<ErasedHierarchyNode>, BevyError>>;

    /// Erased version of [`PointCloudLoader::load_hierarchy`]
    fn load_hierarchy<'a>(
        &'a self,
        node: &'a HierarchyNode,
    ) -> BoxedFuture<'a, Result<Vec<ErasedHierarchyNode>, BevyError>>;

    /// Erased version of [`PointCloudLoader::load_chunk`]
    fn load_chunk<'a>(
        &'a self,
        node: &'a HierarchyNode,
    ) -> BoxedFuture<'a, Result<Mesh, BevyError>>;
}

#[derive(Clone, Debug, Default)]
pub struct ErasedHierarchyNode {
    pub status: HierarchyNodeStatus,
    pub point_count: usize,
    pub child_index: ChildIndex,
    pub parent_id: Option<usize>,
    pub aabb: Option<Aabb>,
    pub data: HierarchyData,
}

impl<H> From<LoadedHierarchyNode<H>> for ErasedHierarchyNode
where
    H: Send + Sync + 'static,
{
    fn from(value: LoadedHierarchyNode<H>) -> Self {
        Self {
            status: value.status,
            point_count: value.point_count,
            child_index: value.child_index,
            parent_id: value.parent_id,
            aabb: value.aabb,
            data: HierarchyData::new(value.data),
        }
    }
}

impl<L: PointCloudLoader> ErasedPointCloudLoader for L {
    fn load_initial_hierarchy<'a>(
        &'a self,
    ) -> BoxedFuture<'a, Result<Vec<ErasedHierarchyNode>, BevyError>> {
        Box::pin(async move {
            let initial_hierarchy = <Self as PointCloudLoader>::load_initial_hierarchy(self)
                .await
                .map_err(Into::into)?;

            Ok(initial_hierarchy
                .into_iter()
                .map(ErasedHierarchyNode::from)
                .collect())
        })
    }

    fn load_hierarchy<'a>(
        &'a self,
        node: &'a HierarchyNode,
    ) -> BoxedFuture<'a, Result<Vec<ErasedHierarchyNode>, BevyError>> {
        Box::pin(async move {
            let Ok(node) = node
                .data
                .deref()
                .clone()
                .downcast::<<Self as PointCloudLoader>::Hierarchy>()
            else {
                return Err("Unable to downcast loaded hierarchy".into());
            };

            let loaded_nodes = self.load_sub_hierarchy(&node).await.map_err(Into::into)?;

            Ok(loaded_nodes
                .into_iter()
                .map(ErasedHierarchyNode::from)
                .collect())
        })
    }

    fn load_chunk<'a>(
        &'a self,
        node: &'a HierarchyNode,
    ) -> BoxedFuture<'a, Result<Mesh, BevyError>> {
        Box::pin(async move {
            let Ok(loaded_hierarchy) = node
                .data
                .deref()
                .clone()
                .downcast::<<Self as PointCloudLoader>::Hierarchy>()
            else {
                return Err("Unable to downcast loaded hierarchy".into());
            };

            self.load_chunk(loaded_hierarchy.as_ref())
                .await
                .map_err(Into::into)
        })
    }
}
