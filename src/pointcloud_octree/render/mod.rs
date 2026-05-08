pub mod data;
pub mod draw;
// pub mod nodes_mapping;
pub mod phase;
pub mod prepare;
pub mod render_node;

pub mod attribute_pass;
pub mod depth_pass;

#[cfg(not(feature = "webgl"))]
pub mod indirect;

#[cfg(not(feature = "webgl"))]
use indirect::{prepare_indirect_buffer, RenderVisibleNodesIndirectBuffers};

use super::asset::extract::PointCloudOctreeNodeUniformLayout;
use bevy_app::prelude::*;
use bevy_camera::Camera3d;
use bevy_ecs::prelude::*;
use bevy_render::{Render, RenderApp, RenderSystems};
use data::{prepare_point_cloud_octree_3d_uniform, PointCloudOctree3dUniformLayout};
use prepare::{
    prepare_visible_nodes_texture, prepare_visible_nodes_texture_bind_group,
    VisibleNodesTextureLayout,
};

#[derive(Default)]
pub struct RenderPointCloudOctreePlugin;

impl Plugin for RenderPointCloudOctreePlugin {
    fn build(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        #[cfg(not(feature = "webgl"))]
        render_app
            .world_mut()
            .register_required_components::<Camera3d, RenderVisibleNodesIndirectBuffers>();

        render_app.add_systems(
            Render,
            (
                #[cfg(not(feature = "webgl"))]
                prepare_indirect_buffer.in_set(RenderSystems::PrepareResources),
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
