#![expect(missing_docs, reason = "Not all docs are written yet.")]

use bevy::{
    app::{App, Plugin},
    asset::embedded_asset,
    camera::visibility::{self, Visibility, VisibilityClass},
    core_pipeline::core_3d::Opaque3d,
    ecs::schedule::IntoScheduleConfigs,
    pbr::MeshPipelineSystems,
    render::{
        extract_component::ExtractComponentPlugin, render_phase::AddRenderCommand,
        render_resource::SpecializedRenderPipelines, Render, RenderApp, RenderStartup,
        RenderSystems,
    },
    transform::components::Transform,
};

mod draw;
mod phase;
mod pipeline;
mod point_cloud;
pub mod prelude;

use draw::*;
use phase::*;
use pipeline::*;
use point_cloud::*;

pub struct PointCloudPlugin;

impl Plugin for PointCloudPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shaders/point_cloud.wgsl");
        app.register_required_components::<PointCloud, Visibility>()
            .register_required_components::<PointCloud, VisibilityClass>()
            .register_required_components::<PointCloud, Transform>()
            .add_plugins(ExtractComponentPlugin::<PointCloud>::default());

        app.world_mut()
            .register_component_hooks::<PointCloud>()
            .on_add(visibility::add_visibility_class::<PointCloud>);

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_render_command::<Opaque3d, DrawPointCloud>()
            .init_resource::<SpecializedRenderPipelines<PointCloudPipeline>>()
            .init_resource::<PendingPointCloudPhaseItemQueues>()
            .add_systems(
                RenderStartup,
                init_point_cloud_pipeline.after(MeshPipelineSystems),
            )
            .add_systems(Render, queue_point_clouds.in_set(RenderSystems::Queue));
    }
}
