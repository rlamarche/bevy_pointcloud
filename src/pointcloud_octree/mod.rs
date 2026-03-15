pub mod asset;
pub mod component;
pub mod extract;
pub mod render;
pub mod visibility;

use crate::octree::OctreeAssetPlugin;
use crate::octree::extract::ExtractVisibleOctreeNodesPlugin;
use crate::octree::server::{OctreeServer, OctreeServerPlugin};
use crate::octree::visibility::OctreeVisiblityPlugin;
use crate::octree::visibility::components::OctreeVisibilitySettings;
use crate::octree::visibility::filter::ScreenPixelRadiusFilter;
use crate::pointcloud_octree::asset::data::PointCloudNodeData;
use crate::pointcloud_octree::extract::RenderPointCloudNodeData;
use crate::pointcloud_octree::visibility::PointCloudOctreePointBudget;
use asset::extract::PointCloudOctreeExtraction;
use bevy_app::plugin_group;
use component::PointCloudOctree3d;

pub type PointCloudOctreeAssetPlugin = OctreeAssetPlugin<PointCloudNodeData>;

pub type PointCloudOctreeVisibilityPlugin = OctreeVisiblityPlugin<
    PointCloudNodeData,
    PointCloudOctree3d,
    ScreenPixelRadiusFilter,
    PointCloudOctreePointBudget,
>;

pub type ExtractVisiblePointCloudOctreeNodesPlugin =
    ExtractVisibleOctreeNodesPlugin<PointCloudOctreeExtraction, RenderPointCloudNodeData>;

pub type PointCloudOctreeVisibilitySettings = OctreeVisibilitySettings<
    PointCloudNodeData,
    ScreenPixelRadiusFilter,
    PointCloudOctreePointBudget,
>;

plugin_group! {
    /// This plugin group will add all the default plugins for a *Bevy* application:
    pub struct PointCloudOctreePlugin {
            self:::PointCloudOctreeAssetPlugin,
            self:::PointCloudOctreeVisibilityPlugin,
            self:::ExtractVisiblePointCloudOctreeNodesPlugin,
            render:::RenderPointCloudOctreePlugin,
    }
}

pub type PointCloudOctreeServer = OctreeServer<PointCloudNodeData>;

pub type PointCloudOctreeServerPlugin = OctreeServerPlugin<PointCloudNodeData>;
