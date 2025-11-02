use crate::octree::OctreeAssetPlugin;
use crate::octree::extract::RenderOctreePlugin;
use crate::pointcloud_octree::asset::PointCloudNodeData;
use crate::pointcloud_octree::component::PointCloudOctree3d;
use crate::pointcloud_octree::extract::RenderPointCloudNodeData;
use crate::pointcloud_octree::render::RenderPointcloudOctreePlugin;
use crate::pointcloud_octree::visibility::{VisiblePointCloudOctree3dNodes, check_octree_node_visibility};
use bevy_app::{App, Plugin, PostUpdate};
use bevy_camera::Camera;
use bevy_camera::prelude::Visibility;
use bevy_camera::visibility::{VisibilityClass, add_visibility_class, check_visibility};
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
        app.register_required_components::<PointCloudOctree3d, Visibility>()
            .register_required_components::<PointCloudOctree3d, VisibilityClass>()
            .add_plugins(PointCloudOctreeAssetPlugin::default())
            .add_plugins(PointCloudOctreeRenderAssetPlugin::default())
            .add_plugins(RenderPointcloudOctreePlugin)
            .register_required_components::<Camera, VisiblePointCloudOctree3dNodes>()
            .add_systems(
                PostUpdate,
                check_octree_node_visibility.after(check_visibility),
            );

        app.world_mut()
            .register_component_hooks::<PointCloudOctree3d>()
            .on_add(add_visibility_class::<PointCloudOctree3d>);
        
        #[cfg(feature = "potree")]
        {
            app.add_plugins(potree::PotreeOctreePlugin);
        }
    }
}
