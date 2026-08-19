mod builder;

use std::ops::Deref;

use bevy::{
    asset::meta::Settings,
    camera::primitives::Aabb,
    ecs::error::BevyError,
    mesh::Mesh,
    tasks::{BoxedFuture, ConditionalSendFuture},
};
use serde::{Deserialize, Serialize};

use crate::PointCloudNode;
pub use builder::*;

/// This structs contains global octree metadata.
/// It must be returned when loading initial hierarchy.
#[derive(Clone, Default, Debug)]
pub struct OctreeMetadata {
    pub point_count: Option<u64>,
    pub aabb: Option<Aabb>,
    pub spacing: Option<f32>,
}

pub trait OctreeLoader: Send + Sync + Sized + 'static {
    /// The source of this loader
    type Source: Send + Sync + 'static;

    /// The settings type used by this [`PointCloudLoader`].
    type Settings: Settings + Default + Serialize + for<'a> Deserialize<'a>;

    /// The data needed to store node level data
    type Hierarchy: Send + Sync + 'static;

    /// The type of [error](`std::error::Error`) which could be encountered by this loader.
    type Error: Into<BevyError>;

    /// This method must return a loader from it's source, it will be called asynchronously
    fn from_source(
        source: Self::Source,
        settings: Self::Settings,
    ) -> impl ConditionalSendFuture<Output = Result<Self, Self::Error>>;

    /// This method must return the octree metadatas. It is always called **before**
    /// [`OctreeLoader::load_initial_hierarchy`].
    fn load_metadata(
        &self,
    ) -> impl ConditionalSendFuture<Output = Result<OctreeMetadata, Self::Error>> {
        Box::pin(async move { Ok(OctreeMetadata::default()) })
    }

    /// This method must build the initial hierarchy of the octree using the provided builder.
    fn load_initial_hierarchy(
        &self,
        builder: &mut OctreeHierarchyBuilder<Self::Hierarchy>,
    ) -> impl ConditionalSendFuture<Output = Result<(), Self::Error>>;

    /// This method must load the provided node sub hierarchy.
    /// The return format is the same as described in [`PointCloudLoader::load_initial_hierarchy`].
    /// So, the provided node is expected to be the first in the returned vector.
    /// The provided node **should** be in [`HierarchyNodeStatus::Proxy`] state, or an error might
    /// be thrown.
    #[expect(
        unused_variables,
        reason = "The parameters here are intentionally unused by the default implementation; however, putting underscores here will result in the underscores being copied by rust-analyzer's tab completion."
    )]
    fn load_sub_hierarchy(
        &self,
        node: &Self::Hierarchy,
        builder: &mut OctreeHierarchyBuilder<Self::Hierarchy>,
    ) -> impl ConditionalSendFuture<Output = Result<(), Self::Error>> {
        Box::pin(async move { Ok(()) })
    }

    /// This method must load the chunk data into a gpu friendly format for rendering.
    /// This format is [`Mesh`] for the moment to reuse Bevy part, it might change in the future.
    fn load_chunk(
        &self,
        node: &Self::Hierarchy,
    ) -> impl ConditionalSendFuture<Output = Result<ChunkLoadResult, Self::Error>>;
}

pub trait ErasedOctreeLoader: Send + Sync + 'static {
    /// Erased version of [`PointCloudLoader::load_metadata`]
    fn load_metadata<'a>(&'a self) -> BoxedFuture<'a, Result<OctreeMetadata, BevyError>>;

    /// Erased version of [`PointCloudLoader::load_initial_hierarchy`]
    fn load_initial_hierarchy<'a>(
        &'a self,
    ) -> BoxedFuture<'a, Result<ErasedOctreeHierarchy, BevyError>>;

    /// Erased version of [`PointCloudLoader::load_hierarchy`]
    fn load_sub_hierarchy<'a>(
        &'a self,
        node: &'a PointCloudNode,
    ) -> BoxedFuture<'a, Result<ErasedOctreeHierarchy, BevyError>>;

    /// Erased version of [`PointCloudLoader::load_chunk`]
    fn load_chunk<'a>(
        &'a self,
        node: &'a PointCloudNode,
    ) -> BoxedFuture<'a, Result<ChunkLoadResult, BevyError>>;
}

impl<L: OctreeLoader> ErasedOctreeLoader for L {
    fn load_metadata<'a>(&'a self) -> BoxedFuture<'a, Result<OctreeMetadata, BevyError>> {
        Box::pin(async move {
            let metadata = <Self as OctreeLoader>::load_metadata(self)
                .await
                .map_err(Into::into)?;

            Ok(metadata)
        })
    }

    fn load_initial_hierarchy<'a>(
        &'a self,
    ) -> BoxedFuture<'a, Result<ErasedOctreeHierarchy, BevyError>> {
        Box::pin(async move {
            let mut builder = OctreeHierarchyBuilder::new();

            <Self as OctreeLoader>::load_initial_hierarchy(self, &mut builder)
                .await
                .map_err(Into::into)?;

            Ok(builder.into())
        })
    }

    fn load_sub_hierarchy<'a>(
        &'a self,
        node: &'a PointCloudNode,
    ) -> BoxedFuture<'a, Result<ErasedOctreeHierarchy, BevyError>> {
        Box::pin(async move {
            let Ok(node) = node
                .data
                .deref()
                .clone()
                .downcast::<<Self as OctreeLoader>::Hierarchy>()
            else {
                return Err("Unable to downcast loaded hierarchy".into());
            };

            let mut builder = OctreeHierarchyBuilder::new();
            self.load_sub_hierarchy(&node, &mut builder)
                .await
                .map_err(Into::into)?;

            Ok(builder.into())
        })
    }

    fn load_chunk<'a>(
        &'a self,
        node: &'a PointCloudNode,
    ) -> BoxedFuture<'a, Result<ChunkLoadResult, BevyError>> {
        Box::pin(async move {
            let Ok(loaded_hierarchy) = node
                .data
                .deref()
                .clone()
                .downcast::<<Self as OctreeLoader>::Hierarchy>()
            else {
                return Err("Unable to downcast loaded hierarchy".into());
            };

            self.load_chunk(loaded_hierarchy.as_ref())
                .await
                .map_err(Into::into)
        })
    }
}

/// Result of a chunk loading operation, allowing dynamic updates.
#[derive(Clone, Default, Debug)]
pub struct ChunkLoadResult {
    /// The actual mesh data. None if the chunk was completely filtered out.
    pub mesh: Option<Mesh>,
    /// offset applied to point size
    pub offset: Option<f32>,
    /// The actual number of points loaded after filtering.
    /// This is crucial to update the memory budget and total point count.
    pub final_point_count: usize,
}
