use crate::potree::loader::PotreeLoader;
use crate::octree::server::{OctreeServerPlugin, OctreeServer};
use crate::pointcloud_octree::asset::data::PointCloudNodeData;
use crate::pointcloud_octree::extract::RenderPointCloudNodeData;
use crate::pointcloud_octree::component::PointCloudOctree3d;

pub mod loader;
pub mod mapping;

pub type PotreeServer = OctreeServer<PotreeLoader, PointCloudNodeData>;

pub type PotreeServerPlugin = OctreeServerPlugin<
    PotreeLoader,
    PointCloudNodeData,
    PointCloudOctree3d,
    RenderPointCloudNodeData,
>;
