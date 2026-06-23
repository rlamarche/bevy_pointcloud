pub mod asset;
pub mod component;
pub mod extract;
pub mod render;
pub mod visibility;

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
    point_cloud::PointCloudGpuMapper,
    pointcloud_octree::{
        asset::{data::PointCloudNodeData, extract::PointCloudOctreeGpuMapper},
        visibility::PointCloudOctreePointBudget,
    },
};

pub type PointCloudOctreeAssetPlugin<T> =
    OctreeAssetPlugin<PointCloudNodeData<T>, PointCloudOctree3d<T>>;

pub type PointCloudOctreeVisibilityPlugin<T> = OctreeVisiblityPlugin<
    PointCloudNodeData<T>,
    PointCloudOctree3d<T>,
    ScreenPixelRadiusFilter,
    PointCloudOctreePointBudget,
>;

#[allow(type_alias_bounds)]
pub type ExtractVisiblePointCloudOctreeNodesPlugin<A: PointCloudGpuMapper> =
    ExtractVisibleOctreeNodesPlugin<PointCloudOctreeGpuMapper<A>>;

pub type PointCloudOctreeVisibilitySettings<T> = OctreeVisibilitySettings<
    PointCloudNodeData<T>,
    ScreenPixelRadiusFilter,
    PointCloudOctreePointBudget,
>;

pub type RenderPointCloudRGBOctreePlugin = render::RenderPointCloudOctreePlugin<RGBPoint>;

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
