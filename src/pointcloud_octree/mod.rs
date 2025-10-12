use crate::octree::OctreeAssetPlugin;
use crate::pointcloud_octree::asset::PointCloudNodeData;

pub mod asset;
pub mod component;

#[cfg(feature = "potree")]
pub mod potree;

pub type PointCloudOctreeAssetPlugin = OctreeAssetPlugin::<PointCloudNodeData>;
