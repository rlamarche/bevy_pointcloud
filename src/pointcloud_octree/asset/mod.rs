use crate::octree::asset::Octree;
use data::PointCloudNodeData;

pub mod data;
pub mod extract;

pub type PointCloudOctree = Octree<PointCloudNodeData>;
