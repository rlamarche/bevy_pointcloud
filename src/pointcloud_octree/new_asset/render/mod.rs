pub mod data;
pub mod draw;
pub mod prepare;
// pub mod node;
pub mod phase;

pub mod attribute_pass;
pub mod depth_pass;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_render::{Render, RenderApp, RenderSystems};
use data::{prepare_point_cloud_octree_3d_uniform, NewPointCloudOctree3dUniformLayout};
use prepare::{
    prepare_octree_nodes_mapping_buffers, prepare_visible_nodes_texture, prepare_visible_nodes_texture_bind_group,
    OctreeNodesMappingBindGroups,
    VisibleNodesTextureLayout,
};

pub struct RenderNewPointCloudOctreePlugin;

impl Plugin for RenderNewPointCloudOctreePlugin {
    fn build(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_systems(
                Render,
                (
                    prepare_visible_nodes_texture.in_set(RenderSystems::PrepareResources),
                    prepare_octree_nodes_mapping_buffers.in_set(RenderSystems::PrepareBindGroups),
                    prepare_visible_nodes_texture_bind_group
                        .in_set(RenderSystems::PrepareBindGroups),
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
        render_app.init_resource::<NewPointCloudOctree3dUniformLayout>();
        render_app.init_resource::<VisibleNodesTextureLayout>();
        render_app.init_resource::<OctreeNodesMappingBindGroups>();
    }
}
