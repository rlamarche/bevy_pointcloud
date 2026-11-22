mod camera;
mod components;
pub mod deferred;
mod draw;
mod extract;
mod key;
mod light;
mod material;
pub mod multipass;
mod node;
mod phase;
mod pipeline;
mod pipeline_specializer;
mod point_cloud;
mod prepare;
pub mod prepass;
pub mod render_phase;
mod resources;

use std::path::PathBuf;

use bevy::{
    app::Plugin,
    asset::{embedded_asset, AssetPath, Assets, Handle, RenderAssetUsages},
    core_pipeline::{
        core_3d::{main_opaque_pass_3d, main_transparent_pass_3d},
        deferred::node::late_deferred_prepass,
        mip_generation::experimental::depth::early_downsample_depth,
        prepass::node::{early_prepass, late_prepass},
        Core3d, Core3dSystems,
    },
    ecs::{
        resource::Resource,
        schedule::{IntoScheduleConfigs, SystemSet},
        world::{FromWorld, World},
    },
    math::primitives::Rectangle,
    mesh::{Mesh, PrimitiveTopology},
    pbr::{
        extract_lights, main_build_indirect_parameters, per_view_shadow_pass, shared_shadow_pass,
        ExtractedDirectionalLight, MeshPipelineSystems,
    },
    render::{
        camera::extract_cameras,
        extract_component::ExtractComponentPlugin,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        init_gpu_resource,
        render_asset::RenderAssetPlugin,
        render_phase::DrawFunctions,
        view::ExtractedView,
        ExtractSchedule, GpuResourceAppExt, Render, RenderApp, RenderDebugFlags, RenderStartup,
        RenderSystems,
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
pub use node::*;
pub use phase::*;
pub use pipeline::*;
pub use pipeline_specializer::*;
pub use point_cloud::*;
pub use prepare::*;
pub use resources::*;

use crate::{
    render::{
        deferred::{
            node::{early_chunk_deferred_prepass, late_chunk_deferred_prepass},
            AlphaMask3dChunkDeferred, Opaque3dChunkDeferred,
        },
        prepass::{
            early_chunk_prepass, extract_camera_prepass_phase, late_chunk_prepass,
            AlphaMask3dChunkPrepass, Opaque3dChunkPrepass,
        },
        render_phase::{BinnedChunksRenderPhasePlugin, ViewBinnedChunksRenderPhases},
    },
    PointCloud3d, PointCloudChunk3d, SplatSettings,
};

#[derive(Default)]
pub struct RenderPointCloudPlugin {
    /// Debugging flags that can optionally be set when constructing the renderer.
    pub debug_flags: RenderDebugFlags,
}

impl RenderPointCloudPlugin {
    pub fn new(debug_flags: RenderDebugFlags) -> Self {
        Self { debug_flags }
    }
}

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
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        // must be initialized before [`MaterialsPlugin`]
        render_app
            .init_resource::<DrawFunctions<Opaque3dChunk>>()
            .init_resource::<DrawFunctions<AlphaMask3dChunk>>()
            .init_resource::<DrawFunctions<Opaque3dChunkDeferred>>()
            .init_resource::<DrawFunctions<AlphaMask3dChunkDeferred>>()
            .init_resource::<DrawFunctions<Opaque3dChunkPrepass>>()
            .init_resource::<DrawFunctions<AlphaMask3dChunkPrepass>>()
            .init_resource::<DrawFunctions<ShadowChunk>>()
            .init_resource::<ViewBinnedChunksRenderPhases<Opaque3dChunk>>()
            .init_resource::<ViewBinnedChunksRenderPhases<AlphaMask3dChunk>>()
            .init_resource::<ViewBinnedChunksRenderPhases<Opaque3dChunkPrepass>>()
            .init_resource::<ViewBinnedChunksRenderPhases<AlphaMask3dChunkPrepass>>()
            .init_resource::<ViewBinnedChunksRenderPhases<Opaque3dChunkDeferred>>()
            .init_resource::<ViewBinnedChunksRenderPhases<AlphaMask3dChunkDeferred>>()
            .init_resource::<ViewBinnedChunksRenderPhases<ShadowChunk>>();

        app.add_plugins(MaterialsPlugin::default()) // TODO add debug flags opt
            .add_plugins(ExtractComponentPlugin::<PointCloud3d>::default())
            .add_plugins(ExtractComponentPlugin::<PointCloudChunk3d>::default())
            .add_plugins(ExtractComponentPlugin::<SplatSettings>::default())
            .add_plugins(ExtractResourcePlugin::<SplatMeshes>::default())
            .add_plugins(RenderAssetPlugin::<RenderPointCloudChunk>::default())
            .add_plugins((
                BinnedChunksRenderPhasePlugin::<Opaque3dChunk, PointCloudPipeline>::new(
                    self.debug_flags,
                ),
                BinnedChunksRenderPhasePlugin::<AlphaMask3dChunk, PointCloudPipeline>::new(
                    self.debug_flags,
                ),
                BinnedChunksRenderPhasePlugin::<Opaque3dChunkDeferred, PointCloudPipeline>::new(
                    self.debug_flags,
                ),
                BinnedChunksRenderPhasePlugin::<AlphaMask3dChunkDeferred, PointCloudPipeline>::new(
                    self.debug_flags,
                ),
                BinnedChunksRenderPhasePlugin::<ShadowChunk, PointCloudPipeline>::new(
                    self.debug_flags,
                ),
            ))
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
                )
                    .chain(),
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
                    extract_camera_phase,
                    extract_camera_prepass_phase,
                    extract_pointcloud_instances
                        .in_set(PointCloudExtractionSystems::ExtractPointClouds),
                    extract_pointcloud_chunk_instances.in_set(PointCloudExtractionSystems::ExtractPointClouds), //.after(extract_meshes_for_cpu_building),
                    extract_lights_visible_point_cloud_chunks
                        .in_set(PointCloudExtractionSystems::ExtractPointClouds)
                        .before(extract_lights),
                    update_render_octree_index
                        .in_set(PointCloudExtractionSystems::ExtractVisiblePointCloudChunks)
                        .after(extract_cameras),
                    // TODO: remove if effectively done in free_removed_point_cloud_materials
                    // free_removed_point_cloud_uniforms,
                ),
            )
            .add_systems(
                Render,
                (
                    (
                        prepare_point_cloud_visible_entities,
                        prepare_shadow_map_point_cloud_visible_entities,
                    )
                        .in_set(RenderSystems::Specialize),
                    (
                        update_point_cloud_visible_entities,
                        update_shadow_map_point_cloud_visible_entities,
                    )
                        .in_set(RenderSystems::Prepare),
                    // [`prepare_cascades_visible_nodes_texture`] after
                    // [`prepare_camera_visible_nodes_texture`] because it can use the same visible
                    // nodes texture TODO: do it
                    (
                        prepare_camera_visible_nodes_texture,
                        prepare_cascades_visible_nodes_texture,
                    )
                        .in_set(RenderSystems::PrepareResources)
                        .chain(),
                    prepare_point_cloud_uniforms.in_set(RenderSystems::PrepareResources),
                    check_views_need_specialization
                        .in_set(RenderSystems::PrepareAssets)
                        .after(bevy::pbr::check_views_need_specialization)
                        .before(RenderSystems::Specialize),
                    mark_visible_nodes_texture_not_changed.in_set(RenderSystems::PrepareBindGroups),
                ),
            )
            .add_systems(
                Core3d,
                main_opaque_pass_3d_chunk
                    .in_set(Core3dSystems::MainPass)
                    .after(main_opaque_pass_3d)
                    .before(main_transparent_pass_3d),
            )
            // prepass
            .add_systems(
                Core3d,
                ((
                    early_chunk_prepass.after(early_prepass),
                    early_chunk_deferred_prepass.after(early_downsample_depth),
                    // TODO: if `early_deferred_prepass` gets public, uncomment the following line
                    // and remove the previous one .after(early_deferred_prepass),
                    late_chunk_prepass.after(late_prepass),
                    late_chunk_deferred_prepass.after(late_deferred_prepass),
                )
                    .chain()
                    .in_set(Core3dSystems::Prepass),),
            )
            // render shadow maps
            .add_systems(
                Core3d,
                (
                    per_view_shadow_chunk_pass::<EARLY_SHADOW_PASS>
                        .after(per_view_shadow_pass::<EARLY_SHADOW_PASS>)
                        .before(early_downsample_depth)
                        .before(per_view_shadow_chunk_pass::<LATE_SHADOW_PASS>),
                    per_view_shadow_chunk_pass::<LATE_SHADOW_PASS>
                        .after(per_view_shadow_pass::<LATE_SHADOW_PASS>)
                        .before(main_build_indirect_parameters)
                        .before(Core3dSystems::MainPass),
                    shared_shadow_chunk_pass::<EARLY_SHADOW_PASS>
                        .after(shared_shadow_pass::<EARLY_SHADOW_PASS>)
                        .before(early_downsample_depth)
                        .before(shared_shadow_pass::<LATE_SHADOW_PASS>),
                    shared_shadow_chunk_pass::<LATE_SHADOW_PASS>
                        .after(shared_shadow_pass::<LATE_SHADOW_PASS>)
                        .before(main_build_indirect_parameters)
                        .before(Core3dSystems::MainPass),
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
        let triangle_mesh = meshes.add(create_equilateral_disk_triangle_mesh(
            0.5,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
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

pub fn shader_ref(path: PathBuf) -> ShaderRef {
    ShaderRef::Path(AssetPath::from_path_buf(path).with_source("embedded"))
}

fn create_equilateral_disk_triangle_mesh(radius: f32, asset_usage: RenderAssetUsages) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, asset_usage);

    // Equilateral triangle centered at (0, 0) with inscribed circle of `radius`.
    let half_base = radius * 3.0_f32.sqrt();
    let y_bottom = -radius;
    let y_top = 2.0 * radius;

    let positions = vec![
        [-half_base, y_bottom, 0.0],
        [half_base, y_bottom, 0.0],
        [0.0, y_top, 0.0],
    ];

    // Centers (0, 0) at UV (0.5, 0.5) so `in.uv - (0.5, 0.5)` matches local `positions`.
    let uvs = positions
        .iter()
        .map(|p| [p[0] + 0.5, p[1] + 0.5])
        .collect::<Vec<_>>();

    let normals = vec![[0.0, 0.0, 1.0]; 3];

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);

    mesh
}
