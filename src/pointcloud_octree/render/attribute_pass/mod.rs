pub mod node;

use super::attribute_pass::node::AttributePassOctreeLabel;
use super::depth_pass::node::DepthPassOctreeLabel;
use super::phase::PointCloudOctree3dBinKey;
use crate::octree::extract::{RenderVisibleOctreeNodes, VisibleOctreeNode};
use crate::pointcloud_octree::asset::data::PointCloudNodeData;
use crate::pointcloud_octree::component::PointCloudOctree3d;
use crate::pointcloud_octree::render::data::SetPointCloudOctree3dUniformGroup;
use crate::pointcloud_octree::render::draw::{DrawPointCloudOctree, DrawPointCloudOctreeNode, SetPointCloudOctreeNodeUniformGroup};
use crate::pointcloud_octree::render::phase::{
    PointCloudOctree3dNodePhase, ViewOctreeNodesRenderAttributePhases,
};
use crate::pointcloud_octree::render::prepare::SetVisibleNodesTexture;
use crate::render::attribute_pass::pipeline::{AttributePassPipeline, AttributePipelineKey};
use crate::render::attribute_pass::texture::prepare_attribute_pass_bind_groups;
use crate::render::material::SetPointCloudMaterialGroup;
use crate::render::normalize_pass::node::NormalizePassLabel;
use crate::render::phase::PointCloud3dBatchSetKey;
use bevy_app::prelude::*;
use bevy_camera::{Camera, Camera3d};
use bevy_core_pipeline::core_3d::graph::Core3d;
use bevy_ecs::component::Tick;
use bevy_ecs::prelude::*;
use bevy_log::prelude::*;
use bevy_pbr::{MeshPipelineKey, SetMeshViewBindGroup};
use bevy_platform::collections::HashSet;
use bevy_render::batching::gpu_preprocessing::GpuPreprocessingSupport;
use bevy_render::render_graph::{RenderGraphExt, ViewNodeRunner};
use bevy_render::render_phase::SetItemPipeline;
use bevy_render::render_resource::{PipelineCache, SpecializedRenderPipelines};
use bevy_render::sync_world::MainEntity;
use bevy_render::view::ExtractedView;
use bevy_render::{
    Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
    prelude::*,
    render_phase::{AddRenderCommand, DrawFunctions},
    view::RetainedViewEntity,
};

pub struct AttributePassPlugin;
impl Plugin for AttributePassPlugin {
    fn build(&self, app: &mut App) {
        // We need to get the render app from the main app
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<DrawFunctions<PointCloudOctree3dNodePhase>>()
            .init_resource::<ViewOctreeNodesRenderAttributePhases<PointCloudOctree3dNodePhase>>()
            .add_render_command::<PointCloudOctree3dNodePhase, DrawAttributePass>()
            .add_systems(ExtractSchedule, extract_camera_phases)
            .add_systems(
                Render,
                (
                    prepare_attribute_pass_bind_groups.in_set(RenderSystems::PrepareResources),
                    queue_attribute_pass.in_set(RenderSystems::QueueMeshes),
                ),
            );

        render_app
            .add_render_graph_node::<ViewNodeRunner<node::AttributePassOctreeNode::<PointCloudOctree3dNodePhase>>>(
                Core3d,
                AttributePassOctreeLabel,
            )
            .add_render_graph_edges(Core3d, (DepthPassOctreeLabel, AttributePassOctreeLabel))
            .add_render_graph_edges(Core3d, (AttributePassOctreeLabel, NormalizePassLabel));
    }
}

// We will reuse render commands already defined by bevy to draw a 3d mesh
type DrawAttributePass = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetPointCloudOctree3dUniformGroup<1>,
    SetPointCloudMaterialGroup<2>,
    SetVisibleNodesTexture<3>,
    SetPointCloudOctreeNodeUniformGroup<4>,
    // SetVisibleOctreeUniformGroup<5>,
    // DrawPointCloudOctreeNode,
    DrawPointCloudOctree,
);

fn extract_camera_phases(
    mut pointcloud3d_phases: ResMut<
        ViewOctreeNodesRenderAttributePhases<PointCloudOctree3dNodePhase>,
    >,
    cameras: Extract<Query<(Entity, &Camera), With<Camera3d>>>,
    mut live_entities: Local<HashSet<RetainedViewEntity>>,
    _gpu_preprocessing_support: Res<GpuPreprocessingSupport>,
) {
    #[cfg(feature = "trace")]
    let _span = info_span!("extract_camera_phases", name = "attribute").entered();
    live_entities.clear();
    for (main_entity, camera) in &cameras {
        if !camera.is_active {
            continue;
        }

        // This is the main camera, so we use the first subview index (0)
        let retained_view_entity = RetainedViewEntity::new(main_entity.into(), None, 0);

        pointcloud3d_phases.prepare_for_new_frame(retained_view_entity);

        live_entities.insert(retained_view_entity);
    }

    // Clear out all dead views.
    pointcloud3d_phases.retain(|camera_entity, _| live_entities.contains(camera_entity));
}

fn queue_attribute_pass(
    custom_draw_functions: Res<DrawFunctions<PointCloudOctree3dNodePhase>>,
    mut pipelines: ResMut<SpecializedRenderPipelines<AttributePassPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    custom_draw_pipeline: Res<AttributePassPipeline>,
    point_cloud_octrees_3d: Query<&PointCloudOctree3d>,
    mut custom_render_phases: ResMut<
        ViewOctreeNodesRenderAttributePhases<PointCloudOctree3dNodePhase>,
    >,
    mut views: Query<(
        &ExtractedView,
        &RenderVisibleOctreeNodes<PointCloudNodeData, PointCloudOctree3d>,
        &Msaa,
    )>,
    main_entities: Query<&MainEntity>,
    mut next_tick: Local<Tick>,
) {
    for (view, visible_entities, msaa) in &mut views {
        let Some(custom_phase) = custom_render_phases.get_mut(&view.retained_view_entity) else {
            continue;
        };
        let draw_custom = custom_draw_functions.read().id::<DrawAttributePass>();

        // Create the key based on the view.
        // In this case we only care about MSAA and HDR
        let view_key = MeshPipelineKey::from_msaa_samples(msaa.samples())
            | MeshPipelineKey::from_hdr(view.hdr);

        let attribute_key = AttributePipelineKey {
            mesh_key: view_key,
            is_octree: true,
        };

        let pipeline_id =
            pipelines.specialize(&pipeline_cache, &custom_draw_pipeline, attribute_key);

        // Since our phase can work on any 3d mesh we can reuse the default mesh 3d filter
        for (render_entity, (_asset_id, visible_octree_nodes)) in &visible_entities.octrees {
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

            // Collect all visible node ids
            let node_ids = visible_octree_nodes
                .iter()
                .map(|VisibleOctreeNode { id: node_id, .. }| *node_id)
                .collect::<Vec<_>>();

            // Add the render phase
            custom_phase.add(
                PointCloud3dBatchSetKey {
                    pipeline: pipeline_id,
                    draw_function: draw_custom,
                },
                PointCloudOctree3dBinKey {
                    asset_id: point_cloud_octree_3d.0.id(),
                },
                (*render_entity, *main_entity),
                node_ids,
            );
        }
    }
}
