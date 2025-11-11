pub mod node;
pub mod phase;

use crate::octree::visibility::{RenderVisibleOctreeNodes, VisibleOctreeNode};
use crate::pointcloud_octree::component::PointCloudOctree3d;
use crate::pointcloud_octree::render::data::SetPointCloudOctree3dUniformGroup;
use crate::pointcloud_octree::render::depth_pass::node::DepthPassOctreeLabel;
use crate::pointcloud_octree::render::draw::{DrawPointCloudOctreeNode, SetPointCloudOctreeNodeUniformGroup};
use crate::pointcloud_octree::render::phase::PointCloudOctree3dBinKey;
use crate::render::attribute_pass::node::AttributePassLabel;
use crate::render::depth_pass::pipeline::{DepthPipeline, DepthPipelineKey};
use crate::render::depth_pass::texture::prepare_depth_pass_textures;
use crate::render::material::SetPointCloudMaterialGroup;
use crate::render::phase::PointCloud3dBatchSetKey;
use crate::render::{PointCloudRenderMode, PointCloudRenderModeOpt};
use bevy_app::prelude::*;
use bevy_camera::{Camera, Camera3d};
use bevy_core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy_ecs::component::Tick;
use bevy_ecs::prelude::*;
use bevy_log::prelude::*;
use bevy_pbr::{MeshPipelineKey, SetMeshViewBindGroup};
use bevy_platform::collections::HashSet;
use bevy_render::batching::gpu_preprocessing::{GpuPreprocessingMode, GpuPreprocessingSupport};
use bevy_render::render_graph::RenderGraphExt;
use bevy_render::render_phase::{BinnedRenderPhaseType, InputUniformIndex, ViewBinnedRenderPhases};
use bevy_render::render_resource::SpecializedRenderPipelines;
use bevy_render::view::NoIndirectDrawing;
use bevy_render::{
    prelude::*, render_graph::ViewNodeRunner, render_phase::{
        AddRenderCommand, DrawFunctions, PhaseItemExtraIndex, SetItemPipeline,
        SortedRenderPhasePlugin, ViewSortedRenderPhases,
    }, render_resource::{CachedRenderPipelineId, PipelineCache, SpecializedMeshPipelines}, sync_world::MainEntity,
    view::{ExtractedView, RenderVisibleEntities, RetainedViewEntity},
    Extract,
    ExtractSchedule,
    Render,
    RenderApp,
    RenderSystems,
};
use phase::PointCloudOctree3dDepthPhase;
use crate::pointcloud_octree::asset::PointCloudNodeData;
use crate::pointcloud_octree::visible_nodes_texture::{SetPointCloudVisibleUniformGroup, SetVisibleNodesTexture};

pub struct DepthPassPlugin;
impl Plugin for DepthPassPlugin {
    fn build(&self, app: &mut App) {
        // We need to get the render app from the main app
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<DrawFunctions<PointCloudOctree3dDepthPhase>>()
            .init_resource::<ViewBinnedRenderPhases<PointCloudOctree3dDepthPhase>>()
            .add_render_command::<PointCloudOctree3dDepthPhase, DrawDepthPass>()
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
            .add_render_graph_node::<ViewNodeRunner<node::DepthPassOctreeNode>>(
                Core3d,
                DepthPassOctreeLabel,
            )
            // Tell the node to run before the main transparent pass
            .add_render_graph_edges(Core3d, (DepthPassOctreeLabel, AttributePassLabel));
    }
}

// We will reuse render commands already defined by bevy to draw a 3d mesh
type DrawDepthPass = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetPointCloudOctree3dUniformGroup<1>,
    SetPointCloudMaterialGroup<2>,
    SetPointCloudOctreeNodeUniformGroup<3>,
    SetVisibleNodesTexture<4>,
    SetPointCloudVisibleUniformGroup<5>,
    DrawPointCloudOctreeNode,
);

fn extract_camera_phases(
    mut pointcloud3d_phases: ResMut<ViewBinnedRenderPhases<PointCloudOctree3dDepthPhase>>,
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
        pointcloud3d_phases
            .get_mut(&retained_view_entity)
            .unwrap()
            .non_mesh_items
            .clear();

        live_entities.insert(retained_view_entity);
    }

    // Clear out all dead views.
    pointcloud3d_phases.retain(|camera_entity, _| live_entities.contains(camera_entity));
}

fn queue_depth_pass(
    custom_draw_functions: Res<DrawFunctions<PointCloudOctree3dDepthPhase>>,
    mut pipelines: ResMut<SpecializedRenderPipelines<DepthPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    custom_draw_pipeline: Res<DepthPipeline>,
    point_cloud_octrees_3d: Query<&PointCloudOctree3d>,
    mut custom_render_phases: ResMut<ViewBinnedRenderPhases<PointCloudOctree3dDepthPhase>>,
    mut views: Query<(
        &ExtractedView,
        &RenderVisibleOctreeNodes<PointCloudNodeData>,
        &Msaa,
        Option<&PointCloudRenderMode>,
    )>,
    main_entities: Query<&MainEntity>,
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
            is_octree: true,
        };

        let pipeline_id = pipelines.specialize(&pipeline_cache, &custom_draw_pipeline, depth_key);

        // Since our phase can work on any 3d mesh we can reuse the default mesh 3d filter
        for (render_entity, visible_octree_nodes) in &visible_entities.octrees {
            let Ok(main_entity) = main_entities.get(*render_entity) else {
                warn!("Render entity not found, skipping.");
                continue;
            };
            let Ok(point_cloud_octree_3d) = point_cloud_octrees_3d.get(*render_entity) else {
                warn!("point_cloud_octree_3d missing");
                continue;
            };
            
            // Bump the change tick in order to force Bevy to rebuild the bin.
            let this_tick = next_tick.get() + 1;
            next_tick.set(this_tick);

            for VisibleOctreeNode { id: node_id, .. } in visible_octree_nodes {
                // At this point we have all the data we need to create a phase item and add it to our
                // phase
                custom_phase.add(
                    PointCloud3dBatchSetKey {
                        pipeline: pipeline_id,
                        draw_function: draw_custom,
                    },
                    PointCloudOctree3dBinKey {
                        asset_id: point_cloud_octree_3d.0.id(),
                        node_id: *node_id,
                    },
                    (*render_entity, *main_entity),
                    InputUniformIndex::default(),
                    BinnedRenderPhaseType::NonMesh,
                    *next_tick,
                );
            }
        }
    }
}
