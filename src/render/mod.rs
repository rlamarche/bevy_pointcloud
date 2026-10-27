mod camera;
mod components;
mod draw;
mod extract;
mod key;
mod light;
mod material;
// mod material_bind_groups;
pub mod multipass;
mod phase;
mod pipeline;
mod pipeline_specializer;
mod point_cloud;
mod prepare;
pub mod prepass;
mod resources;

use std::path::PathBuf;

use bevy::{
    app::Plugin,
    asset::{embedded_asset, AssetPath, Assets, Handle, RenderAssetUsages},
    ecs::{
        resource::Resource,
        schedule::{IntoScheduleConfigs, SystemSet},
        world::{FromWorld, World},
    },
    math::primitives::Rectangle,
    mesh::{Indices, Mesh, PrimitiveTopology},
    pbr::{extract_lights, ExtractedDirectionalLight, MeshPipelineSystems},
    render::{
        camera::extract_cameras,
        extract_component::ExtractComponentPlugin,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        init_gpu_resource,
        render_asset::RenderAssetPlugin,
        view::ExtractedView,
        ExtractSchedule, GpuResourceAppExt, Render, RenderApp, RenderStartup, RenderSystems,
    },
    shader::ShaderRef,
};
pub use camera::*;
pub use components::*;
pub use draw::*;
pub use extract::*;
pub use key::*;
pub use light::*;
pub use material::*;
pub use phase::*;
pub use pipeline::*;
pub use pipeline_specializer::*;
pub use point_cloud::*;
pub use prepare::*;
pub use resources::*;

use crate::{PointCloud3d, PointCloudChunk3d, SplatSettings};

pub struct RenderPointCloudPlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct PointCloudPipelineSystems;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum PointCloudExtractionSystems {
    ExtractPointClouds,
    ExtractVisiblePointCloudChunks,
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
                PointCloudPipelineSystems.after(MeshPipelineSystems),
            )
            .configure_sets(
                ExtractSchedule,
                (
                    PointCloudExtractionSystems::ExtractPointClouds,
                    PointCloudExtractionSystems::ExtractVisiblePointCloudChunks,
                ).chain()
            )
            .init_resource::<RenderPointCloudInstances>()
            .init_resource::<RenderPointCloudChunkInstances>()
            .init_resource::<RenderOctreeInstancesIndex>()
            .init_resource::<PreparedPointCloudUniforms>()
            // unused ?
            // .init_resource::<SpecializedPointCloudPipelines<PointCloudPipeline>>()
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
                    extract_pointcloud_instances
                        .in_set(PointCloudExtractionSystems::ExtractPointClouds),
                    extract_pointcloud_chunk_instances.in_set(PointCloudExtractionSystems::ExtractPointClouds), //.after(extract_meshes_for_cpu_building),
                    extract_lights_visible_point_cloud_chunks
                        .in_set(PointCloudExtractionSystems::ExtractPointClouds)
                        .before(extract_lights),
                    extract_visible_point_cloud_chunks
                        .in_set(PointCloudExtractionSystems::ExtractVisiblePointCloudChunks)
                        .after(extract_cameras),
                    extract_cascade_visible_point_cloud_chunks.in_set(PointCloudExtractionSystems::ExtractVisiblePointCloudChunks)
                        .after(extract_lights),
                    // TODO: remove if effectively done in free_removed_point_cloud_materials
                    // free_removed_point_cloud_uniforms,
                ),
            )
            .add_systems(
                Render,
                (
                    // [`prepare_cascades_visible_nodes_texture`] after [`prepare_camera_visible_nodes_texture`] because it can use the same visible nodes texture
                    // TODO: do it
                    (prepare_camera_visible_nodes_texture, prepare_cascades_visible_nodes_texture)
                        .in_set(RenderSystems::PrepareResources)
                        .chain(),
                    prepare_point_cloud_uniforms.in_set(RenderSystems::PrepareResources),
                    check_views_need_specialization
                        .in_set(RenderSystems::PrepareAssets)
                        .after(bevy::pbr::check_views_need_specialization)
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
            .register_required_components::<ExtractedView, ViewPointCloudBindGroups>();
        render_app
            .world_mut()
            .register_required_components::<ExtractedDirectionalLight, RenderShadowMapVisiblePointCloudEntities>();
    }

    fn finish(&self, app: &mut bevy::app::App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.add_systems(
            RenderStartup,
            init_gpu_resource::<FallbackVisibleNodesTexture>,
        );
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
        let triangle_mesh = meshes.add(create_equilateral_disk_triangle_mesh());

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

pub fn shader_ref(path: PathBuf) -> ShaderRef {
    ShaderRef::Path(AssetPath::from_path_buf(path).with_source("embedded"))
}

fn create_equilateral_disk_triangle_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );

    // Local vertex positions scaled so that the inner UV [0, 1] square
    // produces a disk with a true diameter of 1.0 unit (matching a 1.0x1.0 quad).
    let positions = vec![
        [-1.299, -0.75, 0.0], // Bottom-left vertex
        [1.299, -0.75, 0.0],  // Bottom-right vertex
        [0.0, 1.5, 0.0],      // Top vertex (centered around the disk origin)
    ];

    // UV coordinates mapped to fully enclose the [0, 1] x [0, 1] unit square
    // right in the middle of the triangle.
    let uvs = vec![
        [-0.799, -0.25], // Maps to bottom-left position
        [1.799, -0.25],  // Maps to bottom-right position
        [0.5, 2.0],      // Maps to top position
    ];

    let normals = vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]];

    let indices = vec![0, 1, 2];

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));

    mesh
}
