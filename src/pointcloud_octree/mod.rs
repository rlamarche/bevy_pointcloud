pub mod asset;
pub mod component;
pub mod extract;
pub mod render;
pub mod visibility;
pub mod gpu_mapper;

use component::PointCloudOctree3d;

use crate::{
    octree::{
        OctreeAssetPlugin, extract::{ExtractVisibleOctreeNodesPlugin, OctreeNodesRenderBufferPlugin}, server::{OctreeServer, OctreeServerPlugin}, visibility::{
            OctreeVisiblityPlugin, components::OctreeVisibilitySettings, filter::ScreenPixelRadiusFilter,
        },
    }, point::RGBPoint, point_cloud::PointCloudGpuMapper, point_cloud_material::PointCloudMaterial, pointcloud_octree::{
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

pub type PointCloudOctreeRenderBufferPlugin<T> =
    OctreeNodesRenderBufferPlugin<PointCloudNodeData<T>>;

#[allow(type_alias_bounds)]
pub type ExtractVisiblePointCloudOctreeNodesPlugin<M: PointCloudMaterial, A: PointCloudGpuMapper> =
    ExtractVisibleOctreeNodesPlugin<PointCloudOctreeGpuMapper<M, A>>;

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
