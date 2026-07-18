mod camera;
mod components;
mod draw;
mod extract;
mod light;
mod material;
mod phase;
mod pipeline;
mod pipeline_specializer;
mod point_cloud;
mod point_cloud_bindings;
mod prepare;
mod prepass;
mod resources;

use bevy::{
    app::Plugin,
    asset::{Assets, Handle},
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
    pbr::{extract_meshes_for_cpu_building, MeshPipelineSystems},
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
pub use light::*;
pub use material::*;
pub use phase::*;
pub use pipeline::*;
pub use pipeline_specializer::*;
pub use point_cloud::*;
pub use point_cloud_bindings::*;
pub use prepare::*;
pub use prepass::*;
pub use resources::*;

use crate::{PointCloud3d, PointCloudChunk3d};

pub struct RenderPointCloudPlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct PointCloudPipelineSystems;

impl Plugin for RenderPointCloudPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_plugins(MaterialsPlugin::default()) // TODO add debug flags opt
            .add_plugins(ExtractComponentPlugin::<PointCloud3d>::default())
            .add_plugins(ExtractComponentPlugin::<PointCloudChunk3d>::default())
            .add_plugins(ExtractResourcePlugin::<ShapeMeshes>::default())
            .add_plugins(RenderAssetPlugin::<RenderPointCloudChunk>::default())
            .init_resource::<ShapeMeshes>();

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .configure_sets(
                RenderStartup,
                PointCloudPipelineSystems.after(MeshPipelineSystems),
            )
            .init_resource::<RenderPointCloudInstances>()
            .init_resource::<RenderPointCloudChunkInstances>()
            .init_resource::<RenderPointCloudInstanceIndex>()
            .init_resource::<PreparedPointCloudUniforms>()
            .init_resource::<SpecializedPointCloudPipelines<PointCloudPipeline>>()
            .init_resource::<PendingPointCloudPhaseItemQueues>()
            .add_systems(
                RenderStartup,
                init_point_cloud_pipeline.in_set(PointCloudPipelineSystems),
            )
            .add_systems(
                ExtractSchedule,
                (
                    extract_visible_point_cloud_chunks.after(extract_cameras),
                    extract_pointcloud_instances,
                    extract_pointcloud_chunk_instances.after(extract_meshes_for_cpu_building),
                ),
            )
            .add_systems(
                Render,
                prepare_point_cloud_uniforms.in_set(RenderSystems::PrepareBindGroups),
            )
            .init_gpu_resource::<RenderMaterialBindings>()
            .allow_ambiguous_resource::<RenderMaterialBindings>();

        render_app
            .world_mut()
            .register_required_components::<ExtractedView, RenderVisiblePointCloudEntities>();
    }
}

#[derive(Resource, Clone)]
pub struct ShapeMeshes {
    pub quad_mesh: Handle<Mesh>,
    pub triangle_mesh: Handle<Mesh>,
}

impl FromWorld for ShapeMeshes {
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

impl ExtractResource for ShapeMeshes {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}
