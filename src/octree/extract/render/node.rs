use bevy_asset::AssetId;
use bevy_ecs::{
    prelude::*,
    system::{SystemParam, SystemParamItem},
};
use bevy_render::render_resource::AsBindGroupError;
use thiserror::Error;

use super::{asset::RenderOctreeNodeData, buffer::RenderNodeData};
use crate::octree::{asset::Octree, node::NodeData, storage::NodeId};

#[derive(Debug, Error)]
pub enum PrepareOctreeNodeError<T: Send + Sync> {
    #[error("Failed to prepare asset")]
    RetryNextUpdate(RenderOctreeNodeData<T>),
    #[error("Failed to build bind group: {0}")]
    AsBindGroupError(AsBindGroupError),
}

/// Describes how an octree node gets extracted and prepared for rendering.
///
/// In the [`ExtractSchedule`] step the [`RenderOctreeNode::SourceOctreeNode`] is transferred
/// from the "main world" into the "render world".
///
/// After that in the [`RenderSystems::PrepareAssets`] step the extracted octree nodes
/// are transformed into their GPU-representation of type [`RenderOctreeNode`].
pub trait RenderOctreeNode: Send + Sync + Sized + 'static {
    type SourceOctreeNode: NodeData;

    type ExtractedOctreeNode: RenderNodeData;

    /// Specifies all ECS data required by [`RenderAsset::prepare_asset`].
    ///
    /// For convenience use the [`lifetimeless`](bevy_ecs::system::lifetimeless) [`SystemParam`].
    type Param: SystemParam;

    /// Size of the data the asset will upload to the gpu. Specifying a return value
    /// will allow the asset to be throttled via [`RenderOctreeNodesBytesPerFrame`].
    #[inline]
    #[expect(
        unused_variables,
        reason = "The parameters here are intentionally unused by the default implementation; however, putting underscores here will result in the underscores being copied by rust-analyzer's tab completion."
    )]
    fn byte_len(source_node: &RenderOctreeNodeData<Self::ExtractedOctreeNode>) -> Option<usize> {
        None
    }

    /// Prepares the [`RenderAsset::SourceAsset`] for the GPU by transforming it into a [`RenderAsset`].
    ///
    /// ECS data may be accessed via `param`.
    #[allow(clippy::result_large_err)]
    fn prepare_octree_node(
        source_node: RenderOctreeNodeData<Self::ExtractedOctreeNode>,
        asset_id: AssetId<Octree<Self::SourceOctreeNode>>,
        param: &mut SystemParamItem<Self::Param>,
    ) -> Result<Self, PrepareOctreeNodeError<Self::ExtractedOctreeNode>>;

    /// Called whenever the [`RenderOctreeNode::SourceOctreeNode`] has been removed.
    ///
    /// You can implement this method if you need to access ECS data (via
    /// `_param`) in order to perform cleanup tasks when the asset is removed.
    ///
    /// The default implementation does nothing.
    #[expect(
        unused_variables,
        reason = "The parameters here are intentionally unused by the default implementation; however, putting underscores here will result in the underscores being copied by rust-analyzer's tab completion."
    )]
    fn unload_octree_node(
        source_asset: AssetId<Octree<Self::SourceOctreeNode>>,
        node_id: NodeId,
        param: &mut SystemParamItem<Self::Param>,
    ) {
    }
}
