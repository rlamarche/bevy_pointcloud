use crate::octree::render_asset::NodeId;
use crate::pointcloud_octree::asset::PointCloudOctree;
use bevy_asset::AssetId;

pub trait PointCloudOctree3dPhase {
    fn node_id(&self) -> NodeId;
}

/// Data that must be identical in order to *batch* phase items together.
///
/// Note that a *batch set* (if multi-draw is in use) contains multiple batches.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PointCloudOctree3dBinKey {
    /// The asset that this phase item is associated with.
    ///
    /// Normally, this is the ID of the mesh, but for non-mesh items it might be
    /// the ID of another type of asset.
    pub asset_id: AssetId<PointCloudOctree>,
    pub node_id: NodeId,

}
