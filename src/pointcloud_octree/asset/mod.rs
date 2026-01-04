use crate::octree::asset::Octree;
use data::PointCloudNodeData;

pub mod extract;
pub mod data;

pub type PointCloudOctree = Octree<PointCloudNodeData>;
