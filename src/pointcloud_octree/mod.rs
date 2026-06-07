pub mod asset;
pub mod component;
pub mod extract;
pub mod render;
pub mod visibility;

use asset::extract::PointCloudOctreeExtraction;
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

pub type PointCloudOctreeAssetPlugin<T> = OctreeAssetPlugin<PointCloudNodeData<T>>;

pub type PointCloudOctreeVisibilityPlugin<T> = OctreeVisiblityPlugin<
    PointCloudNodeData<T>,
    PointCloudOctree3d<T>,
    ScreenPixelRadiusFilter,
    PointCloudOctreePointBudget,
>;

pub type ExtractVisiblePointCloudOctreeNodesPlugin<T, U> = ExtractVisibleOctreeNodesPlugin<
    PointCloudOctreeExtraction<T, U>,
    RenderPointCloudNodeData<T, U>,
>;

pub type PointCloudOctreeVisibilitySettings<T> = OctreeVisibilitySettings<
    PointCloudNodeData<T>,
    ScreenPixelRadiusFilter,
    PointCloudOctreePointBudget,
>;

pub type RenderPointCloudRGBOctreePlugin =
    render::RenderPointCloudOctreePlugin<RGBPoint, RGBPoint, SimplePointCloudMaterial>;

// plugin_group! {
//     /// This plugin group will add all the default plugins for a *Bevy* application:
//     pub struct PointCloudOctreePlugin {
//             self:::PointCloudOctreeAssetPlugin,
//             self:::PointCloudOctreeVisibilityPlugin,
//             self:::ExtractVisiblePointCloudOctreeNodesPlugin,
//             self:::RenderPointCloudRGBOctreePlugin,
//     }
// }

pub type PointCloudOctreeServer<T> = OctreeServer<PointCloudNodeData<T>>;

pub type PointCloudOctreeServerPlugin<T> = OctreeServerPlugin<PointCloudNodeData<T>>;
