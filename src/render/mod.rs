mod camera;
mod components;
mod draw;
mod extract;
mod key;
mod light;
mod material;
// mod material_bind_groups;
mod phase;
mod pipeline;
mod pipeline_specializer;
mod point_cloud;
mod prepare;
mod prepass;
mod resources;

use bevy::{
    app::Plugin,
    asset::{embedded_asset, Assets, Handle},
    ecs::{
        resource::Resource,
        schedule::{IntoScheduleConfigs, SystemSet},
        world::{FromWorld, World},
    },
    math::{
        primitives::{Rectangle, Triangle3d},
        Vec3,
    },
    mesh::Mesh,
    pbr::{extract_lights, ExtractedDirectionalLight, MeshPipelineSystems},
    render::{
        camera::extract_cameras,
        extract_component::ExtractComponentPlugin,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::RenderAssetPlugin,
        view::ExtractedView,
        ExtractSchedule, GpuResourceAppExt, Render, RenderApp, RenderStartup, RenderSystems,
    },
};
pub use camera::*;
pub use components::*;
pub use draw::*;
pub use extract::*;
pub use key::*;
pub use light::*;
pub use material::*;
// pub use material_bind_groups::*;
pub use phase::*;
pub use pipeline::*;
pub use pipeline_specializer::*;
pub use point_cloud::*;
pub use prepare::*;
pub use prepass::*;
pub use resources::*;

use crate::{PointCloud3d, PointCloudChunk3d, SplatSettings};

pub struct RenderPointCloudPlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct PointCloudPipelineSystems;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum PointCloudExtractionSystems {
    ExtractPointClouds,
    ExtractPointCloudChunks,
    ExtractVisiblePointCloudChunks,
    ExtractCascadeVisiblePointCloudChunks,
}

impl Plugin for RenderPointCloudPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        embedded_asset!(app, "pointcloud.wgsl");

        app.add_plugins(MaterialsPlugin::default()) // TODO add debug flags opt
            .add_plugins(ExtractComponentPlugin::<PointCloud3d>::default())
            .add_plugins(ExtractComponentPlugin::<PointCloudChunk3d>::default())
            .add_plugins(ExtractComponentPlugin::<SplatSettings>::default())
            .add_plugins(ExtractResourcePlugin::<SplatMeshes>::default())
            .add_plugins(RenderAssetPlugin::<RenderPointCloudChunk>::default())
            .init_resource::<SplatMeshes>();

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .configure_sets(
                RenderStartup,
                (
                    PointCloudPipelineSystems.after(MeshPipelineSystems),
                    PointCloudExtractionSystems::ExtractVisiblePointCloudChunks
                        .after(extract_cameras),
                    PointCloudExtractionSystems::ExtractCascadeVisiblePointCloudChunks
                        .before(extract_lights),
                ),
            )
            .init_resource::<RenderPointCloudInstances>()
            .init_resource::<RenderPointCloudChunkInstances>()
            .init_resource::<RenderOctreeInstancesIndex>()
            .init_resource::<PreparedPointCloudUniforms>()
            .init_resource::<SpecializedPointCloudPipelines<PointCloudPipeline>>()
            .init_resource::<PendingPointCloudPhaseItemQueues>()
            .add_systems(
                RenderStartup,
                (
                    init_point_cloud_pipeline.in_set(PointCloudPipelineSystems),
                    // init_fallback_bindless_resources,
                ),
            )
            .add_systems(
                ExtractSchedule,
                (
                    extract_visible_point_cloud_chunks
                        .in_set(PointCloudExtractionSystems::ExtractVisiblePointCloudChunks),
                    extract_cascade_visible_point_cloud_chunks.in_set(PointCloudExtractionSystems::ExtractVisiblePointCloudChunks),
                    extract_pointcloud_instances
                        .in_set(PointCloudExtractionSystems::ExtractPointClouds),
                    extract_pointcloud_chunk_instances.in_set(PointCloudExtractionSystems::ExtractPointCloudChunks), //.after(extract_meshes_for_cpu_building),
                    extract_lights_visible_point_cloud_chunks
                        .in_set(PointCloudExtractionSystems::ExtractCascadeVisiblePointCloudChunks)
                        .before(extract_lights)
                    ,
                ),
            )
            .add_systems(
                Render,
                (
                    prepare_visible_nodes_texture.in_set(RenderSystems::PrepareResources),
                    prepare_visible_nodes_texture_for_lights.in_set(RenderSystems::PrepareResources),
                    prepare_visible_nodes_texture_bind_group
                        .in_set(RenderSystems::PrepareBindGroups),
                    prepare_point_cloud_uniforms.in_set(RenderSystems::PrepareBindGroups),
                    check_views_need_specialization
                        .after(RenderSystems::PrepareAssets)
                        .before(RenderSystems::Specialize),
                ),
            )
            .init_gpu_resource::<RenderMaterialBindings>()
            .allow_ambiguous_resource::<RenderMaterialBindings>();

        render_app
            .world_mut()
            .register_required_components::<ExtractedView, RenderVisiblePointCloudEntities>();

        render_app
            .world_mut()
            .register_required_components::<ExtractedDirectionalLight, RenderShadowMapVisiblePointCloudEntities>();
    }
}

#[derive(Resource, Clone)]
pub struct SplatMeshes {
    pub quad_mesh: Handle<Mesh>,
    pub triangle_mesh: Handle<Mesh>,
}

impl FromWorld for SplatMeshes {
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

impl ExtractResource for SplatMeshes {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}
