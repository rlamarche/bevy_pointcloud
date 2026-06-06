use std::hash::Hash;

use bevy_asset::AssetId;
use bevy_render::{
    render_phase::{DrawFunctionId, PhaseItemBatchSetKey},
    render_resource::CachedRenderPipelineId,
};

use crate::{point::Point, point_cloud::PointCloud};

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
#[derive(Clone, Debug)]
pub struct PointCloud3dBinKey<T: Point> {
    /// The asset that this phase item is associated with.
    ///
    /// Normally, this is the ID of the mesh, but for non-mesh items it might be
    /// the ID of another type of asset.
    pub asset_id: AssetId<PointCloud<T>>,
}

impl<T: Point> Hash for PointCloud3dBinKey<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.asset_id.hash(state);
    }
}

impl<T: Point> PartialEq for PointCloud3dBinKey<T> {
    fn eq(&self, other: &Self) -> bool {
        self.asset_id == other.asset_id
    }
}

impl<T: Point> PartialOrd for PointCloud3dBinKey<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Point> Eq for PointCloud3dBinKey<T> {}

impl<T: Point> Ord for PointCloud3dBinKey<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.asset_id.cmp(&other.asset_id)
    }
}
