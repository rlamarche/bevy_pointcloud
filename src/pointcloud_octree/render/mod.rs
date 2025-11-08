pub mod data;
pub mod draw;
// pub mod node;
pub mod phase;
pub mod visibility;

pub mod attribute_pass;
pub mod depth_pass;

use crate::pointcloud_octree::component::PointCloudOctree3d;
use crate::pointcloud_octree::render::data::{
    prepare_point_cloud_octree_3d_uniform, PointCloudOctree3dUniformLayout,
};
use crate::pointcloud_octree::render::visibility::RenderVisiblePointCloudOctree3dNodes;
use crate::pointcloud_octree::visibility::VisiblePointCloudOctree3dNodes;
use bevy_app::prelude::*;
use bevy_camera::Camera;
use bevy_ecs::prelude::*;
use bevy_render::camera::extract_cameras;
use bevy_render::extract_component::ExtractComponentPlugin;
use bevy_render::prelude::*;
use bevy_render::sync_world::RenderEntity;
use bevy_render::{Extract, Render, RenderApp, RenderSystems};

pub struct RenderPointcloudOctreePlugin;

impl Plugin for RenderPointcloudOctreePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<PointCloudOctree3d>::default());

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_systems(
                ExtractSchedule,
                (extract_visible_point_cloud_octree_3d_nodes.after(extract_cameras),),
            )
            .add_systems(
                Render,
                (prepare_point_cloud_octree_3d_uniform.in_set(RenderSystems::PrepareResources),),
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
        render_app.init_resource::<PointCloudOctree3dUniformLayout>();
    }
}

fn extract_visible_point_cloud_octree_3d_nodes(
    mut commands: Commands,
    query: Extract<
        Query<(
            Entity,
            RenderEntity,
            &Camera,
            &VisiblePointCloudOctree3dNodes,
        )>,
    >,
    mapper: Extract<Query<&RenderEntity>>,
) {
    for (_entity, render_entity, camera, visible_point_cloud_octree_3d_nodes) in query.iter() {
        let render_visible_point_cloud_octree_3d_nodes = RenderVisiblePointCloudOctree3dNodes {
            octrees: visible_point_cloud_octree_3d_nodes
                .nodes
                .clone()
                .into_iter()
                .map(|(entity, nodes)| {
                    let render_entity = mapper
                        .get(entity)
                        .expect("Render entity for PointCloudOctree3d not found");
                    (render_entity.id(), nodes)
                })
                .collect(),
        };
        commands
            .entity(render_entity)
            .insert(render_visible_point_cloud_octree_3d_nodes);
    }
}
