pub mod data;
pub mod draw;
// pub mod nodes_mapping;
pub mod phase;
pub mod prepare;
pub mod render_node;

pub mod attribute_pass;
pub mod depth_pass;

use super::asset::extract::PointCloudOctreeNodeUniformLayout;
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_render::{Render, RenderApp, RenderSystems};
use data::{PointCloudOctree3dUniformLayout, prepare_point_cloud_octree_3d_uniform};
use prepare::{
    VisibleNodesTextureLayout, prepare_visible_nodes_texture,
    prepare_visible_nodes_texture_bind_group,
};

#[derive(Default)]
pub struct RenderPointCloudOctreePlugin;

impl Plugin for RenderPointCloudOctreePlugin {
    fn build(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.add_systems(
            Render,
            (
                prepare_visible_nodes_texture.in_set(RenderSystems::PrepareResources),
                // nodes_mapping::prepare_octree_nodes_mapping_buffers.in_set(RenderSystems::PrepareBindGroups),
                prepare_visible_nodes_texture_bind_group.in_set(RenderSystems::PrepareBindGroups),
                prepare_point_cloud_octree_3d_uniform.in_set(RenderSystems::PrepareResources),
            ),
        );

        app.add_plugins((
            depth_pass::DepthPassPlugin,
            attribute_pass::AttributePassPlugin,
        ));
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.init_resource::<PointCloudOctreeNodeUniformLayout>();
        render_app.init_resource::<PointCloudOctree3dUniformLayout>();
        render_app.init_resource::<VisibleNodesTextureLayout>();
        // render_app.init_resource::<nodes_mapping::OctreeNodesMappingBindGroups>();
    }
}
