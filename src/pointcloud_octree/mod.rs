use crate::octree::OctreeAssetPlugin;
use crate::octree::visibility::ExtractVisibleOctreeNodesPlugin;
use crate::pointcloud_octree::asset::PointCloudNodeData;
use crate::pointcloud_octree::component::PointCloudOctree3d;
use crate::pointcloud_octree::extract::{
    PointCloudOctreeNodeUniformLayout, RenderPointCloudNodeData,
};
use crate::pointcloud_octree::render::RenderPointCloudOctreePlugin;
use crate::pointcloud_octree::visible_nodes_texture::{OctreeNodesMappingBindGroups, VisibleNodesTextureLayout, prepare_visible_nodes_texture, prepare_visible_nodes_texture_bind_group, prepare_octree_nodes_mapping_buffers};
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_render::{Render, RenderApp, RenderSystems};

pub mod asset;
pub mod component;

pub mod extract;
#[cfg(feature = "potree")]
pub mod potree;
pub mod render;

pub mod visible_nodes_texture;

pub type PointCloudOctreeAssetPlugin = OctreeAssetPlugin<PointCloudNodeData>;

pub type ExtractVisiblePointCloudOctreeNodesPlugin = ExtractVisibleOctreeNodesPlugin<
    PointCloudNodeData,
    PointCloudOctree3d,
    RenderPointCloudNodeData,
>;

pub struct PointCloudOctreePlugin;

impl Plugin for PointCloudOctreePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PointCloudOctreeAssetPlugin::default())
            // .add_plugins(PointCloudOctreeRenderNodeDataPlugin::default())
            // .add_plugins(PointCloudOctreeRenderNodeUniformPlugin::default())
            .add_plugins(RenderPointCloudOctreePlugin)
            .add_plugins(ExtractVisiblePointCloudOctreeNodesPlugin::default());

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(
                Render,
                (
                    prepare_visible_nodes_texture.in_set(RenderSystems::PrepareResources),
                    prepare_octree_nodes_mapping_buffers.in_set(RenderSystems::PrepareBindGroups),
                    prepare_visible_nodes_texture_bind_group
                        .in_set(RenderSystems::PrepareBindGroups),
                ),
            );
        }

        #[cfg(feature = "potree")]
        {
            app.add_plugins(potree::PotreeOctreePlugin);
        }
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.init_resource::<PointCloudOctreeNodeUniformLayout>();
        render_app.init_resource::<VisibleNodesTextureLayout>();
        render_app.init_resource::<OctreeNodesMappingBindGroups>();
    }
}
