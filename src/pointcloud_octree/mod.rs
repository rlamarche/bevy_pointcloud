use crate::octree::OctreeAssetPlugin;
use crate::octree::extract::RenderOctreePlugin;
use crate::pointcloud_octree::asset::PointCloudNodeData;
use bevy_app::{App, Plugin};
use crate::pointcloud_octree::extract::RenderPointCloudNodeData;

pub mod asset;
pub mod component;

pub mod extract;
#[cfg(feature = "potree")]
pub mod potree;

pub type PointCloudOctreeAssetPlugin = OctreeAssetPlugin<PointCloudNodeData>;
pub type PointCloudOctreeRenderAssetPlugin = RenderOctreePlugin<RenderPointCloudNodeData>;

pub struct PointCloudOctreePlugin;

impl Plugin for PointCloudOctreePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            PointCloudOctreeAssetPlugin::default(),
            PointCloudOctreeRenderAssetPlugin::default(),
        ));
    }
}
