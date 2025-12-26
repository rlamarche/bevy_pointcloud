use crate::octree::new_asset::asset::NewOctree;
use crate::octree::new_asset::extract::ExtractVisibleOctreeNodesPlugin;
use crate::octree::new_asset::visibility::NewOctreeVisiblityPlugin;
use crate::octree::new_asset::NewOctreeAssetPlugin;
use crate::pointcloud_octree::asset::PointCloudNodeData;
use crate::pointcloud_octree::extract::RenderPointCloudNodeData;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_render::RenderApp;
use component::NewPointCloudOctree3d;
use extract::{NewPointCloudOctreeExtraction, PointCloudOctreeNodeUniformLayout};
use render::RenderNewPointCloudOctreePlugin;

pub mod component;
pub mod extract;
pub mod render;

pub type NewPointCloudOctree = NewOctree<PointCloudNodeData>;

pub type NewPointCloudOctreeAssetPlugin = NewOctreeAssetPlugin<PointCloudNodeData>;

pub type NewPointCloudOctreeVisibilityPlugin =
    NewOctreeVisiblityPlugin<PointCloudNodeData, NewPointCloudOctree3d, RenderPointCloudNodeData>;

pub type NewExtractVisibleOctreeNodesPlugin =
    ExtractVisibleOctreeNodesPlugin<NewPointCloudOctreeExtraction, RenderPointCloudNodeData>;

pub struct NewPointCloudOctreePlugin;

impl Plugin for NewPointCloudOctreePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            NewPointCloudOctreeAssetPlugin::default(),
            NewPointCloudOctreeVisibilityPlugin::default(),
            NewExtractVisibleOctreeNodesPlugin::default(),
            RenderNewPointCloudOctreePlugin,
        ));
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.init_resource::<PointCloudOctreeNodeUniformLayout>();
    }
}
