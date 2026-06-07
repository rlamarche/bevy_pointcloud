pub mod node;

use std::marker::PhantomData;

use bevy_app::prelude::*;
use bevy_camera::{Camera, Camera3d};
use bevy_core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy_ecs::{change_detection::Tick, prelude::*};
use bevy_log::prelude::*;
use bevy_pbr::{MeshPipelineKey, SetMeshViewBindGroup};
use bevy_platform::collections::HashSet;
use bevy_render::{
    batching::gpu_preprocessing::GpuPreprocessingSupport,
    prelude::*,
    render_graph::{RenderGraphExt, ViewNodeRunner},
    render_phase::{AddRenderCommand, DrawFunctions, SetItemPipeline},
    render_resource::{PipelineCache, SpecializedRenderPipelines},
    sync_world::MainEntity,
    view::{ExtractedView, RetainedViewEntity},
    Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
};
use node::DepthPassOctreeLabel;

use super::phase::PointCloudOctree3dBinKey;
#[cfg(feature = "webgl")]
use crate::pointcloud_octree::render::draw::DrawPointCloudOctree;
#[cfg(not(feature = "webgl"))]
use crate::pointcloud_octree::render::draw::DrawPointCloudOctreeIndirect;
use crate::{
    octree::extract::render::components::RenderVisibleOctreeNodes,
    point::{GpuPoint, Point},
    pointcloud_octree::{
        asset::data::PointCloudNodeData,
        component::PointCloudOctree3d,
        render::{
            data::SetPointCloudOctree3dUniformGroup,
            draw::{SetPointCloudOctreeNodeUniformGroup, SetRenderOctreeUniformGroup},
            phase::{PointCloudOctree3dNodePhase, ViewOctreeNodesRenderDepthPhases},
            prepare::SetVisibleNodesTexture,
        },
    },
    render::{
        depth_pass::{
            pipeline::{DepthPipeline, DepthPipelineKey},
            texture::prepare_depth_pass_textures,
        },
        material::SetPointCloudMaterialGroup,
        phase::PointCloud3dBatchSetKey,
        PointCloudRenderMode, PointCloudRenderModeOpt,
    },
    PointCloudMaterial,
};

pub struct DepthPassPlugin<T: Point, U: GpuPoint, M: PointCloudMaterial>(
    #[allow(clippy::type_complexity)] PhantomData<fn() -> (T, U, M)>,
);

impl<T: Point, U: GpuPoint, M: PointCloudMaterial> Default for DepthPassPlugin<T, U, M> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: Point, U: GpuPoint, M: PointCloudMaterial> Plugin for DepthPassPlugin<T, U, M> {
    fn build(&self, app: &mut App) {
        // We need to get the render app from the main app
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<DrawFunctions<PointCloudOctree3dNodePhase<T>>>()
            .init_resource::<ViewOctreeNodesRenderDepthPhases<PointCloudOctree3dNodePhase<T>>>()
            .add_render_command::<PointCloudOctree3dNodePhase<T>, DrawDepthPass<T, U, M>>()
            .init_resource::<SpecializedRenderPipelines<DepthPipeline<T, U, M>>>()
            .add_systems(ExtractSchedule, extract_camera_phases::<T>)
            .add_systems(
                Render,
                (
                    prepare_depth_pass_textures.in_set(RenderSystems::PrepareResources),
                    queue_depth_pass::<T, U, M>.in_set(RenderSystems::QueueMeshes),
                ),
            );

        render_app
            .add_render_graph_node::<ViewNodeRunner<node::DepthPassOctreeNode::<PointCloudOctree3dNodePhase<T>>>>(
                Core3d,
                DepthPassOctreeLabel,
            )
            // Tell the node to run before the main transparent pass
            .add_render_graph_edges(Core3d, (DepthPassOctreeLabel, Node3d::MainTransparentPass));
    }
}

// We will reuse render commands already defined by bevy to draw a 3d mesh
#[cfg(not(feature = "webgl"))]
type DrawDepthPass<T, U, M> = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetPointCloudOctree3dUniformGroup<1>,
    SetPointCloudMaterialGroup<2, M>,
    SetVisibleNodesTexture<3>,
    SetPointCloudOctreeNodeUniformGroup<4, T, U>,
    SetRenderOctreeUniformGroup<5, T, U>,
    DrawPointCloudOctreeIndirect<T, U>,
);

#[cfg(feature = "webgl")]
type DrawDepthPass<M> = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetPointCloudOctree3dUniformGroup<1>,
    SetPointCloudMaterialGroup<2, M>,
    SetVisibleNodesTexture<3>,
    SetPointCloudOctreeNodeUniformGroup<4, T, U>,
    SetRenderOctreeUniformGroup<5, T, U>,
    DrawPointCloudOctree<T, U>,
);

fn extract_camera_phases<T: Point>(
    mut pointcloud3d_phases: ResMut<
        ViewOctreeNodesRenderDepthPhases<PointCloudOctree3dNodePhase<T>>,
    >,
    cameras: Extract<Query<(Entity, &Camera), With<Camera3d>>>,
    mut live_entities: Local<HashSet<RetainedViewEntity>>,
    _gpu_preprocessing_support: Res<GpuPreprocessingSupport>,
) {
    #[cfg(feature = "trace")]
    let _span = info_span!("extract_camera_phases", name = "depth").entered();
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

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
fn queue_depth_pass<T: Point, U: GpuPoint, M: PointCloudMaterial>(
    custom_draw_functions: Res<DrawFunctions<PointCloudOctree3dNodePhase<T>>>,
    mut pipelines: ResMut<SpecializedRenderPipelines<DepthPipeline<T, U, M>>>,
    pipeline_cache: Res<PipelineCache>,
    custom_draw_pipeline: Res<DepthPipeline<T, U, M>>,
    point_cloud_octrees_3d: Query<&PointCloudOctree3d<T>>,
    mut custom_render_phases: ResMut<
        ViewOctreeNodesRenderDepthPhases<PointCloudOctree3dNodePhase<T>>,
    >,
    mut views: Query<(
        &ExtractedView,
        &RenderVisibleOctreeNodes<PointCloudNodeData<T>, PointCloudOctree3d<T>>,
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
        let draw_custom = custom_draw_functions.read().id::<DrawDepthPass<T, U, M>>();

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
        for (render_entity, _) in &visible_entities.octrees {
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
            );
        }
    }
}
