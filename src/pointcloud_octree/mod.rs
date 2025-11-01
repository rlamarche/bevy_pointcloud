use crate::octree::OctreeAssetPlugin;
use crate::octree::extract::RenderOctreePlugin;
use crate::pointcloud_octree::asset::PointCloudNodeData;
use crate::pointcloud_octree::extract::RenderPointCloudNodeData;
use crate::pointcloud_octree::render::RenderPointcloudOctreePlugin;
use crate::pointcloud_octree::visibility::{VisibleOctreeNodes, check_octree_node_visibility};
use bevy_app::{App, Plugin, PostUpdate};
use bevy_camera::Camera;
use bevy_camera::visibility::check_visibility;
use bevy_ecs::prelude::*;

pub mod asset;
pub mod component;

pub mod extract;
#[cfg(feature = "potree")]
pub mod potree;
pub mod render;
mod visibility;

pub type PointCloudOctreeAssetPlugin = OctreeAssetPlugin<PointCloudNodeData>;
pub type PointCloudOctreeRenderAssetPlugin = RenderOctreePlugin<RenderPointCloudNodeData>;

pub struct PointCloudOctreePlugin;

impl Plugin for PointCloudOctreePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            PointCloudOctreeAssetPlugin::default(),
            PointCloudOctreeRenderAssetPlugin::default(),
            RenderPointcloudOctreePlugin,
        ))
        .register_required_components::<Camera, VisibleOctreeNodes>()
        .add_systems(
            PostUpdate,
            check_octree_node_visibility.after(check_visibility),
        );
        #[cfg(feature = "potree")]
        {
            app.add_plugins(potree::PotreeOctreePlugin);
        }
    }
}
