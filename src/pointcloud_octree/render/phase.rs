use bevy_asset::{AssetId, UntypedAssetId};
use bevy_ecs::prelude::*;
use bevy_render::render_phase::{BinnedPhaseItem, CachedRenderPipelinePhaseItem, DrawFunctionId, PhaseItem, PhaseItemBatchSetKey, PhaseItemExtraIndex};
use bevy_render::render_resource::CachedRenderPipelineId;
use bevy_render::sync_world::MainEntity;
use std::ops::Range;
use crate::octree::render_asset::NodeId;
use crate::pointcloud_octree::asset::PointCloudOctree;

pub struct Pointcloud3d {
    /// Determines which objects can be placed into a *batch set*.
    ///
    /// Objects in a single batch set can potentially be multi-drawn together,
    /// if it's enabled and the current platform supports it.
    pub batch_set_key: Pointcloud3dBatchSetKey,
    /// The key, which determines which can be batched.
    pub bin_key: Pointcloud3dBinKey,
    /// An entity from which data will be fetched, including the mesh if
    /// applicable.
    pub representative_entity: (Entity, MainEntity),
    /// The ranges of instances.
    pub batch_range: Range<u32>,
    /// An extra index, which is either a dynamic offset or an index in the
    /// indirect parameters list.
    pub extra_index: PhaseItemExtraIndex,
}

/// Information that must be identical in order to place opaque meshes in the
/// same *batch set*.
///
/// A batch set is a set of batches that can be multi-drawn together, if
/// multi-draw is in use.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pointcloud3dBatchSetKey {
    pub pipeline: CachedRenderPipelineId,
    pub draw_function: DrawFunctionId,
}

impl PhaseItemBatchSetKey for Pointcloud3dBatchSetKey {
    fn indexed(&self) -> bool {
        false
    }
}

/// Data that must be identical in order to *batch* phase items together.
///
/// Note that a *batch set* (if multi-draw is in use) contains multiple batches.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pointcloud3dBinKey {
    /// The asset that this phase item is associated with.
    ///
    /// Normally, this is the ID of the mesh, but for non-mesh items it might be
    /// the ID of another type of asset.
    pub asset_id: AssetId<PointCloudOctree>,
    pub node_id: NodeId,

}

impl PhaseItem for Pointcloud3d {
    #[inline]
    fn entity(&self) -> Entity {
        self.representative_entity.0
    }

    #[inline]
    fn main_entity(&self) -> MainEntity {
        self.representative_entity.1
    }

    #[inline]
    fn draw_function(&self) -> DrawFunctionId {
        self.batch_set_key.draw_function
    }

    #[inline]
    fn batch_range(&self) -> &Range<u32> {
        &self.batch_range
    }

    #[inline]
    fn batch_range_mut(&mut self) -> &mut Range<u32> {
        &mut self.batch_range
    }

    fn extra_index(&self) -> PhaseItemExtraIndex {
        self.extra_index.clone()
    }

    fn batch_range_and_extra_index_mut(&mut self) -> (&mut Range<u32>, &mut PhaseItemExtraIndex) {
        (&mut self.batch_range, &mut self.extra_index)
    }
}

impl BinnedPhaseItem for Pointcloud3d {
    type BinKey = Pointcloud3dBinKey;
    type BatchSetKey = Pointcloud3dBatchSetKey;

    #[inline]
    fn new(
        batch_set_key: Self::BatchSetKey,
        bin_key: Self::BinKey,
        representative_entity: (Entity, MainEntity),
        batch_range: Range<u32>,
        extra_index: PhaseItemExtraIndex,
    ) -> Self {
        Pointcloud3d {
            batch_set_key,
            bin_key,
            representative_entity,
            batch_range,
            extra_index,
        }
    }
}

impl CachedRenderPipelinePhaseItem for Pointcloud3d {
    #[inline]
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.batch_set_key.pipeline
    }
}
