use crate::new_potree::loader::PotreeLoader;
use crate::octree::new_asset::server::{NewOctreeServerPlugin, OctreeServer};
use crate::pointcloud_octree::asset::PointCloudNodeData;
use crate::pointcloud_octree::extract::RenderPointCloudNodeData;
use crate::pointcloud_octree::new_asset::component::NewPointCloudOctree3d;

pub mod loader;

pub type PotreeServer = OctreeServer<PotreeLoader, PointCloudNodeData>;

pub type PotreeServerPlugin = NewOctreeServerPlugin<
    PotreeLoader,
    PointCloudNodeData,
    NewPointCloudOctree3d,
    RenderPointCloudNodeData,
>;
