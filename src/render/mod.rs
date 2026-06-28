mod components;
mod draw;
mod extract;
mod phase;
mod pipeline;
mod point_cloud;
mod point_cloud_bindings;
mod resources;

use bevy::{
    app::Plugin,
    asset::{embedded_asset, Assets, Handle},
    core_pipeline::core_3d::Opaque3d,
    ecs::{
        resource::Resource,
        schedule::IntoScheduleConfigs,
        world::{FromWorld, World},
    },
    math::{
        primitives::{Rectangle, Triangle3d},
        Vec3,
    },
    mesh::Mesh,
    pbr::MeshPipelineSystems,
    render::{
        camera::extract_cameras,
        extract_component::ExtractComponentPlugin,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::RenderAssetPlugin,
        render_phase::AddRenderCommand,
        render_resource::SpecializedRenderPipelines,
        view::ExtractedView,
        ExtractSchedule, Render, RenderApp, RenderStartup, RenderSystems,
    },
};
pub use components::*;
pub use draw::*;
pub use extract::*;
pub use phase::*;
pub use pipeline::*;
pub use point_cloud::*;
pub use point_cloud_bindings::*;
pub use resources::*;

use crate::{PointCloud3d, PointCloudChunk3d};

pub struct RenderPointCloudPlugin;

impl Plugin for RenderPointCloudPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        embedded_asset!(app, "shaders/point_cloud.wgsl");
        app.add_plugins(ExtractComponentPlugin::<PointCloud3d>::default())
            .add_plugins(ExtractComponentPlugin::<PointCloudChunk3d>::default())
            .add_plugins(ExtractResourcePlugin::<PointMeshes>::default())
            .add_plugins(RenderAssetPlugin::<RenderPointCloudChunk>::default())
            .init_resource::<PointMeshes>();

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<RenderPointCloudInstanceIndex>()
            .add_render_command::<Opaque3d, DrawPointCloud>()
            .init_resource::<SpecializedRenderPipelines<PointCloudPipeline>>()
            .init_resource::<PendingPointCloudPhaseItemQueues>()
            .add_systems(
                RenderStartup,
                init_point_cloud_pipeline.after(MeshPipelineSystems),
            )
            .add_systems(
                ExtractSchedule,
                extract_visible_point_cloud_chunks.after(extract_cameras),
            )
            .add_systems(
                Render,
                (
                    queue_point_clouds.in_set(RenderSystems::Queue),
                    collect_visible_cpu_culled_point_cloud_chunk_entities
                        .in_set(RenderSystems::PrepareAssets),
                ),
            );

        render_app
            .world_mut()
            .register_required_components::<ExtractedView, RenderVisiblePointCloudEntities>();
    }
}

#[derive(Resource, Clone)]
pub struct PointMeshes {
    pub quad_mesh: Handle<Mesh>,
    pub triangle_mesh: Handle<Mesh>,
}

impl FromWorld for PointMeshes {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();

        let quad_mesh = meshes.add(Rectangle::new(1.0, 1.0));
        let triangle_mesh = meshes.add(Triangle3d::new(
            Vec3::new(-1.0, -0.577, 0.0),
            Vec3::new(1.0, -0.577, 0.0),
            Vec3::new(0.0, 1.155, 0.0),
        ));

        Self {
            quad_mesh,
            triangle_mesh,
        }
    }
}

impl ExtractResource for PointMeshes {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}
