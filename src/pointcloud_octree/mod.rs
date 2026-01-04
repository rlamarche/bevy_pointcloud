pub mod asset;
pub mod extract;
pub mod render;
pub mod component;

use crate::octree::extract::ExtractVisibleOctreeNodesPlugin;
use crate::octree::visibility::OctreeVisiblityPlugin;
use crate::octree::OctreeAssetPlugin;
use crate::pointcloud_octree::asset::data::PointCloudNodeData;
use crate::pointcloud_octree::extract::RenderPointCloudNodeData;
use component::PointCloudOctree3d;
use asset::extract::{PointCloudOctreeExtraction, PointCloudOctreeNodeUniformLayout};
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_render::RenderApp;
use render::RenderPointCloudOctreePlugin;

pub type PointCloudOctreeAssetPlugin = OctreeAssetPlugin<PointCloudNodeData>;

pub type PointCloudOctreeVisibilityPlugin =
    OctreeVisiblityPlugin<PointCloudNodeData, PointCloudOctree3d, RenderPointCloudNodeData>;

pub type ExtractVisiblePointCloudOctreeNodesPlugin =
    ExtractVisibleOctreeNodesPlugin<PointCloudOctreeExtraction, RenderPointCloudNodeData>;

pub struct PointCloudOctreePlugin;

impl Plugin for PointCloudOctreePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            PointCloudOctreeAssetPlugin::default(),
            PointCloudOctreeVisibilityPlugin::default(),
            ExtractVisiblePointCloudOctreeNodesPlugin::default(),
            RenderPointCloudOctreePlugin,
        ));
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.init_resource::<PointCloudOctreeNodeUniformLayout>();
    }
}
