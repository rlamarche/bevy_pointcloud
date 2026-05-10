use data::PointCloudNodeData;

use crate::octree::asset::Octree;

pub mod data;
pub mod extract;

pub type PointCloudOctree = Octree<PointCloudNodeData>;
