use std::hash::Hash;

use bevy_asset::AssetId;
use bevy_render::{
    render_phase::{DrawFunctionId, PhaseItemBatchSetKey},
    render_resource::CachedRenderPipelineId,
};

use crate::point_cloud::PointCloud;

/// Information that must be identical in order to place opaque meshes in the
/// same *batch set*.
///
/// A batch set is a set of batches that can be multi-drawn together, if
/// multi-draw is in use.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PointCloud3dBatchSetKey {
    pub pipeline: CachedRenderPipelineId,
    pub draw_function: DrawFunctionId,
}

impl PhaseItemBatchSetKey for PointCloud3dBatchSetKey {
    fn indexed(&self) -> bool {
        false
    }
}

/// Data that must be identical in order to *batch* phase items together.
///
/// Note that a *batch set* (if multi-draw is in use) contains multiple batches.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PointCloud3dBinKey {
    /// The asset that this phase item is associated with.
    ///
    /// Normally, this is the ID of the mesh, but for non-mesh items it might be
    /// the ID of another type of asset.
    pub asset_id: AssetId<PointCloud>,
}
