pub mod asset;
pub mod component;
pub mod extract;
pub mod render;
pub mod visibility;

use asset::extract::PointCloudOctreeExtraction;
use bevy_app::plugin_group;
use component::PointCloudOctree3d;

use crate::{
    octree::{
        extract::ExtractVisibleOctreeNodesPlugin,
        server::{OctreeServer, OctreeServerPlugin},
        visibility::{
            components::OctreeVisibilitySettings, filter::ScreenPixelRadiusFilter,
            OctreeVisiblityPlugin,
        },
        OctreeAssetPlugin,
    },
    point::RGBPoint,
    pointcloud_octree::{
        asset::data::PointCloudNodeData, extract::RenderPointCloudNodeData,
        visibility::PointCloudOctreePointBudget,
    },
    SimplePointCloudMaterial,
};

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

pub type RenderPointCloudRGBOctreePlugin =
    render::RenderPointCloudOctreePlugin<RGBPoint, RGBPoint, SimplePointCloudMaterial>;

plugin_group! {
    /// This plugin group will add all the default plugins for a *Bevy* application:
    pub struct PointCloudOctreePlugin {
            self:::PointCloudOctreeAssetPlugin,
            self:::PointCloudOctreeVisibilityPlugin,
            self:::ExtractVisiblePointCloudOctreeNodesPlugin,
            self:::RenderPointCloudRGBOctreePlugin,
    }
}

pub type PointCloudOctreeServer = OctreeServer<PointCloudNodeData>;

pub type PointCloudOctreeServerPlugin = OctreeServerPlugin<PointCloudNodeData>;
