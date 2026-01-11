pub mod asset;
pub mod component;
pub mod extract;
pub mod render;
pub mod visibility;

use crate::octree::OctreeAssetPlugin;
use crate::octree::eviction::OctreeEvictionPlugin;
use crate::octree::extract::ExtractVisibleOctreeNodesPlugin;
use crate::octree::server::{OctreeServer, OctreeServerPlugin};
use crate::octree::visibility::OctreeVisiblityPlugin;
use crate::octree::visibility::components::OctreeVisibilitySettings;
use crate::octree::visibility::filter::ScreenPixelRadiusFilter;
use crate::pointcloud_octree::asset::data::PointCloudNodeData;
use crate::pointcloud_octree::extract::RenderPointCloudNodeData;
use crate::pointcloud_octree::visibility::PointCloudOctreePointBudget;
use asset::extract::{PointCloudOctreeExtraction, PointCloudOctreeNodeUniformLayout};
use bevy_app::{App, Plugin};
use bevy_render::RenderApp;
use component::PointCloudOctree3d;
use render::RenderPointCloudOctreePlugin;

pub type PointCloudOctreeAssetPlugin = OctreeAssetPlugin<PointCloudNodeData>;

pub type PointCloudOctreeVisibilityPlugin = OctreeVisiblityPlugin<
    PointCloudNodeData,
    PointCloudOctree3d,
    ScreenPixelRadiusFilter,
    PointCloudOctreePointBudget,
>;

pub type ExtractVisiblePointCloudOctreeNodesPlugin =
    ExtractVisibleOctreeNodesPlugin<PointCloudOctreeExtraction, RenderPointCloudNodeData>;

pub struct PointCloudOctreePlugin;

pub type PointCloudOctreeVisibilitySettings = OctreeVisibilitySettings<
    PointCloudNodeData,
    ScreenPixelRadiusFilter,
    PointCloudOctreePointBudget,
>;

pub type PointCloudOctreeEvictionPlugin =
    OctreeEvictionPlugin<PointCloudNodeData, PointCloudOctree3d>;

impl Plugin for PointCloudOctreePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            PointCloudOctreeAssetPlugin::default(),
            PointCloudOctreeVisibilityPlugin::default(),
            PointCloudOctreeEvictionPlugin::default(),
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

pub type PointCloudOctreeServer = OctreeServer<PointCloudNodeData>;

pub type PointCloudOctreeServerPlugin =
    OctreeServerPlugin<PointCloudNodeData, PointCloudOctree3d, RenderPointCloudNodeData>;
