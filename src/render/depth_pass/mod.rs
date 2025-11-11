pub mod node;
pub mod phase;
pub mod pipeline;
pub mod texture;

use crate::point_cloud::PointCloud3d;
use crate::render::depth_pass::node::{DepthPassLabel, DepthPassNode};
use crate::render::depth_pass::pipeline::{DepthPipeline, DepthPipelineKey};
use crate::render::depth_pass::texture::{DepthPassLayout, prepare_depth_pass_textures};
use crate::render::draw::DrawPointCloud;
use crate::render::material::SetPointCloudMaterialGroup;
use crate::render::phase::{PointCloud3dBatchSetKey, PointCloud3dBinKey};
use crate::render::point_cloud_uniform::SetPointCloudUniformGroup;
use crate::render::{PointCloudRenderMode, PointCloudRenderModeOpt};
use bevy_app::prelude::*;
use bevy_camera::{Camera, Camera3d};
use bevy_core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy_ecs::component::Tick;
use bevy_ecs::prelude::*;
use bevy_log::prelude::*;
use bevy_pbr::{
    MeshPipelineKey, SetMeshViewBindGroup,
};
use bevy_platform::collections::HashSet;
use bevy_render::batching::gpu_preprocessing::{GpuPreprocessingMode, GpuPreprocessingSupport};
use bevy_render::render_graph::RenderGraphExt;
use bevy_render::render_phase::{BinnedRenderPhaseType, InputUniformIndex, ViewBinnedRenderPhases};
use bevy_render::render_resource::SpecializedRenderPipelines;
use bevy_render::view::NoIndirectDrawing;
use bevy_render::{
    Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
    prelude::*,
    render_graph::ViewNodeRunner,
    render_phase::{
        AddRenderCommand, DrawFunctions, SetItemPipeline,
    },
    render_resource::PipelineCache,
    view::{ExtractedView, RenderVisibleEntities, RetainedViewEntity},
};
use phase::PointCloud3dDepthPhase;

pub struct DepthPassPlugin;
impl Plugin for DepthPassPlugin {
    fn build(&self, app: &mut App) {
        // app.add_plugins(SortedRenderPhasePlugin::<DepthPass3d, MeshPipeline>::new(
        //     RenderDebugFlags::default(),
        // ));

        // We need to get the render app from the main app
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<DrawFunctions<PointCloud3dDepthPhase>>()
            .init_resource::<ViewBinnedRenderPhases<PointCloud3dDepthPhase>>()
            .add_render_command::<PointCloud3dDepthPhase, DrawDepthPass>()
            .init_resource::<SpecializedRenderPipelines<DepthPipeline>>()
            .add_systems(ExtractSchedule, extract_camera_phases)
            .add_systems(
                Render,
                (
                    prepare_depth_pass_textures.in_set(RenderSystems::PrepareResources),
                    queue_depth_pass.in_set(RenderSystems::QueueMeshes),
                ),
            );

        render_app
            .add_render_graph_node::<ViewNodeRunner<DepthPassNode>>(Core3d, DepthPassLabel)
            // Tell the node to run before the main transparent pass
            .add_render_graph_edges(Core3d, (DepthPassLabel, Node3d::MainTransparentPass));
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        // The pipeline needs the RenderDevice to be created and it's only available once plugins
        // are initialized
        render_app
            .init_resource::<DepthPassLayout>()
            .init_resource::<DepthPipeline>();
    }
}

// We will reuse render commands already defined by bevy to draw a 3d mesh
type DrawDepthPass = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetPointCloudUniformGroup<1>,
    SetPointCloudMaterialGroup<2>,
    DrawPointCloud,
);

fn extract_camera_phases(
    mut pointcloud3d_phases: ResMut<ViewBinnedRenderPhases<PointCloud3dDepthPhase>>,
    cameras: Extract<Query<(Entity, &Camera, Has<NoIndirectDrawing>), With<Camera3d>>>,
    mut live_entities: Local<HashSet<RetainedViewEntity>>,
    gpu_preprocessing_support: Res<GpuPreprocessingSupport>,
) {
    live_entities.clear();
    for (main_entity, camera, no_indirect_drawing) in &cameras {
        if !camera.is_active {
            continue;
        }

        // If GPU culling is in use, use it (and indirect mode); otherwise, just
        // preprocess the meshes.
        let gpu_preprocessing_mode = gpu_preprocessing_support.min(if !no_indirect_drawing {
            GpuPreprocessingMode::Culling
        } else {
            GpuPreprocessingMode::PreprocessingOnly
        });

        // This is the main camera, so we use the first subview index (0)
        let retained_view_entity = RetainedViewEntity::new(main_entity.into(), None, 0);

        pointcloud3d_phases.prepare_for_new_frame(retained_view_entity, gpu_preprocessing_mode);

        // clear phases between each iteration
        pointcloud3d_phases.get_mut(&retained_view_entity).unwrap().non_mesh_items.clear();

        live_entities.insert(retained_view_entity);
    }

    // Clear out all dead views.
    pointcloud3d_phases.retain(|camera_entity, _| live_entities.contains(camera_entity));
}

fn queue_depth_pass(
    custom_draw_functions: Res<DrawFunctions<PointCloud3dDepthPhase>>,
    mut pipelines: ResMut<SpecializedRenderPipelines<DepthPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    custom_draw_pipeline: Res<DepthPipeline>,
    point_clouds_3d: Query<&PointCloud3d>,
    mut custom_render_phases: ResMut<ViewBinnedRenderPhases<PointCloud3dDepthPhase>>,
    mut views: Query<(
        &ExtractedView,
        &RenderVisibleEntities,
        &Msaa,
        Option<&PointCloudRenderMode>,
    )>,
    mut next_tick: Local<Tick>,
) {
    for (view, visible_entities, msaa, point_cloud_render_mode) in &mut views {
        let Some(custom_phase) = custom_render_phases.get_mut(&view.retained_view_entity) else {
            continue;
        };
        let draw_custom = custom_draw_functions.read().id::<DrawDepthPass>();

        // Create the key based on the view.
        // In this case we only care about MSAA and HDR
        let view_key = MeshPipelineKey::from_msaa_samples(msaa.samples())
            | MeshPipelineKey::from_hdr(view.hdr);

        let depth_key = DepthPipelineKey {
            mesh_key: view_key,
            use_edl: point_cloud_render_mode.use_edl(),
            is_octree: false,
        };

        let pipeline_id =
            pipelines.specialize(&pipeline_cache, &custom_draw_pipeline, depth_key);

        // Since our phase can work on any 3d mesh we can reuse the default mesh 3d filter
        for (render_entity, main_entity) in visible_entities.iter::<PointCloud3d>() {
            let Ok(point_cloud__3d) = point_clouds_3d.get(*render_entity) else {
                warn!("point_cloud_3d missing");
                continue;
            };

            // Bump the change tick in order to force Bevy to rebuild the bin.
            let this_tick = next_tick.get() + 1;
            next_tick.set(this_tick);

            // At this point we have all the data we need to create a phase item and add it to our
            // phase
            custom_phase.add(
                PointCloud3dBatchSetKey {
                    pipeline: pipeline_id,
                    draw_function: draw_custom,
                },
                PointCloud3dBinKey {
                    asset_id: point_cloud__3d.0.id(),
                },
                (*render_entity, *main_entity),
                InputUniformIndex::default(),
                BinnedRenderPhaseType::NonMesh,
                *next_tick,
            );
        }
    }
}
