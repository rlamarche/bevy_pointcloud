use crate::new_potree::component::NewPotreePointCloud3d;
use crate::new_potree::extract::PotreeExtraction;
use crate::new_potree::loader::{PotreeHierarchy, PotreeLoader};
use crate::octree::new_asset::NewOctreeAssetPlugin;
use crate::octree::new_asset::asset::NewOctree;
use crate::octree::new_asset::extract::ExtractVisibleOctreeNodesPlugin;
use crate::octree::new_asset::server::{NewOctreeServerPlugin, OctreeServer};
use crate::octree::new_asset::visibility::NewOctreeVisiblityPlugin;
use crate::pointcloud_octree::asset::PointCloudNodeData;
use crate::pointcloud_octree::extract::RenderPointCloudNodeData;

pub mod component;
mod extract;
pub mod loader;

pub type PotreeServer = OctreeServer<PotreeLoader, PotreeHierarchy, PointCloudNodeData>;

pub type PotreeAssetPlugin = NewOctreeAssetPlugin<PotreeHierarchy, PointCloudNodeData>;

pub type PotreeServerPlugin = NewOctreeServerPlugin<
    PotreeLoader,
    PotreeHierarchy,
    PointCloudNodeData,
    NewPotreePointCloud3d,
    RenderPointCloudNodeData,
>;

pub type PotreeAsset = NewOctree<PotreeHierarchy, PointCloudNodeData>;

pub type PotreeVisibilityPlugin = NewOctreeVisiblityPlugin<
    PotreeLoader,
    PotreeHierarchy,
    PointCloudNodeData,
    NewPotreePointCloud3d,
    RenderPointCloudNodeData,
>;

pub type PotreeExtractVisibleOctreeNodesPlugin =
    ExtractVisibleOctreeNodesPlugin<PotreeExtraction, RenderPointCloudNodeData>;
