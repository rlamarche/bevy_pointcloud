use crate::octree::OctreeAssetPlugin;
use crate::octree::extract::RenderOctreePlugin;
use crate::octree::visibility::ExtractVisibleOctreeNodesPlugin;
use crate::pointcloud_octree::asset::PointCloudNodeData;
use crate::pointcloud_octree::component::PointCloudOctree3d;
use crate::pointcloud_octree::extract::{
    PointCloudNodeUniformLayout, RenderPointCloudNodeData, RenderPointCloudNodeUniform,
};
use crate::pointcloud_octree::render::RenderPointCloudOctreePlugin;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_render::RenderApp;

pub mod asset;
pub mod component;

pub mod extract;
#[cfg(feature = "potree")]
pub mod potree;
pub mod render;

pub type PointCloudOctreeAssetPlugin = OctreeAssetPlugin<PointCloudNodeData>;
// pub type PointCloudOctreeRenderNodeDataPlugin = RenderOctreePlugin<RenderPointCloudNodeData>;
// pub type PointCloudOctreeRenderNodeUniformPlugin = RenderOctreePlugin<RenderPointCloudNodeUniform>;

pub type ExtractVisiblePointCloudOctreeNodesPlugin =
    ExtractVisibleOctreeNodesPlugin<PointCloudNodeData, PointCloudOctree3d, RenderPointCloudNodeData>;

pub struct PointCloudOctreePlugin;

impl Plugin for PointCloudOctreePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PointCloudOctreeAssetPlugin::default())
            // .add_plugins(PointCloudOctreeRenderNodeDataPlugin::default())
            // .add_plugins(PointCloudOctreeRenderNodeUniformPlugin::default())
            .add_plugins(RenderPointCloudOctreePlugin)
            .add_plugins(ExtractVisiblePointCloudOctreeNodesPlugin::default());

        #[cfg(feature = "potree")]
        {
            app.add_plugins(potree::PotreeOctreePlugin);
        }
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.init_resource::<PointCloudNodeUniformLayout>();
    }
}
