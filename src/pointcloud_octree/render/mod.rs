pub mod data;
pub mod draw;
mod mesh;
pub mod node;
pub mod phase;
pub mod pipeline;
pub mod visibility;

use crate::octree::extract::RenderOctrees;
use crate::pointcloud_octree::component::PointCloudOctree3d;
use crate::pointcloud_octree::extract::RenderPointCloudNodeData;
use crate::pointcloud_octree::render::data::{
    PointCloudOctree3dUniformLayout, prepare_point_cloud_octree_3d_uniform,
};
use crate::pointcloud_octree::render::draw::DrawPointCloudOctree;
use crate::pointcloud_octree::render::mesh::PointCloudOctreeMesh;
use crate::pointcloud_octree::render::node::{PointCloudOctree3dDrawNode, Pointcloud3dDrawLabel};
use crate::pointcloud_octree::render::phase::{
    PointCloudOctree3dBatchSetKey, PointCloudOctree3dBinKey, PointCloudOctree3dPhase,
};
use crate::pointcloud_octree::render::pipeline::{
    PointCloudOctreePipeline, PointCloudOctreePipelineKey,
};
use crate::pointcloud_octree::render::visibility::RenderVisiblePointCloudOctree3dNodes;
use crate::pointcloud_octree::visibility::VisiblePointCloudOctree3dNodes;
use bevy_app::prelude::*;
use bevy_asset::prelude::*;
use bevy_asset::{load_internal_asset, uuid_handle};
use bevy_camera::{Camera, Camera3d};
use bevy_core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy_ecs::component::Tick;
use bevy_ecs::prelude::*;
use bevy_log::prelude::*;
use bevy_pbr::MeshPipelineKey;
use bevy_platform::collections::HashSet;
use bevy_render::batching::gpu_preprocessing::{GpuPreprocessingMode, GpuPreprocessingSupport};
use bevy_render::camera::extract_cameras;
use bevy_render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy_render::prelude::*;
use bevy_render::render_graph::{RenderGraphExt, ViewNodeRunner};
use bevy_render::render_phase::{
    AddRenderCommand, BinnedRenderPhaseType, DrawFunctions, InputUniformIndex,
    ViewBinnedRenderPhases,
};
use bevy_render::render_resource::{PipelineCache, SpecializedRenderPipelines};
use bevy_render::sync_world::{MainEntity, RenderEntity};
use bevy_render::view::{ExtractedView, NoIndirectDrawing, RetainedViewEntity};
use bevy_render::{Extract, Render, RenderApp, RenderSystems};
use bevy_shader::prelude::*;

const POINTCLOUD_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("b0c157c1-7ab6-42fb-ac39-432e52901acd");

// const NORMALIZE_SHADER_HANDLE: Handle<Shader> =
//     uuid_handle!("95fb019e-64e3-4155-807c-53d866664238");

pub struct RenderPointcloudOctreePlugin;

impl Plugin for RenderPointcloudOctreePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            POINTCLOUD_SHADER_HANDLE,
            "point_cloud.wgsl",
            Shader::from_wgsl
        );

        app.add_plugins(ExtractComponentPlugin::<PointCloudOctree3d>::default());

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<DrawFunctions<PointCloudOctree3dPhase>>()
            .add_render_command::<PointCloudOctree3dPhase, DrawPointCloudOctree>()
            .init_resource::<ViewBinnedRenderPhases<PointCloudOctree3dPhase>>()
            .init_resource::<SpecializedRenderPipelines<PointCloudOctreePipeline>>()
            .add_systems(
                ExtractSchedule,
                (
                    extract_camera_phases,
                    // extract_point_cloud_octree_3d,
                    extract_visible_point_cloud_octree_3d_nodes.after(extract_cameras),
                    // .after(extract_point_cloud_octree_3d),
                ),
            )
            .add_systems(
                Render,
                (
                    prepare_point_cloud_octree_3d_uniform.in_set(RenderSystems::PrepareResources),
                    queue_attribute_pass.in_set(RenderSystems::Queue),
                ),
            )
            .add_render_graph_node::<ViewNodeRunner<PointCloudOctree3dDrawNode>>(
                Core3d,
                Pointcloud3dDrawLabel,
            )
            .add_render_graph_edges(Core3d, (Pointcloud3dDrawLabel, Node3d::MainTransparentPass));
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        // The pipeline needs the RenderDevice to be created and it's only available once plugins
        // are initialized
        render_app
            .init_resource::<PointCloudOctree3dUniformLayout>()
            .init_resource::<PointCloudOctreeMesh>()
            .init_resource::<PointCloudOctreePipeline>();
    }
}

// using custom system here to be able to add system's ordering constraints
// fn extract_point_cloud_octree_3d(
//     mut commands: Commands,
//     mut previous_len: Local<usize>,
//     query: Extract<
//         Query<
//             (
//                 RenderEntity,
//                 <PointCloudOctree3d as ExtractComponent>::QueryData,
//             ),
//             <PointCloudOctree3d as ExtractComponent>::QueryFilter,
//         >,
//     >,
// ) {
//     let mut values = Vec::with_capacity(*previous_len);
//     for (entity, query_item) in &query {
//         if let Some(component) =
//             <PointCloudOctree3d as ExtractComponent>::extract_component(query_item)
//         {
//             values.push((entity, component));
//         } else {
//             commands
//                 .entity(entity)
//                 .remove::<<PointCloudOctree3d as ExtractComponent>::Out>();
//         }
//     }
//     *previous_len = values.len();
//     commands.try_insert_batch(values);
// }

fn extract_visible_point_cloud_octree_3d_nodes(
    mut commands: Commands,
    query: Extract<
        Query<(
            Entity,
            RenderEntity,
            &Camera,
            &VisiblePointCloudOctree3dNodes,
        )>,
    >,
    mapper: Extract<Query<&RenderEntity>>,
) {
    for (_entity, render_entity, camera, visible_point_cloud_octree_3d_nodes) in query.iter() {
        let render_visible_point_cloud_octree_3d_nodes = RenderVisiblePointCloudOctree3dNodes {
            octrees: visible_point_cloud_octree_3d_nodes
                .nodes
                .clone()
                .into_iter()
                .map(|(entity, nodes)| {
                    let render_entity = mapper
                        .get(entity)
                        .expect("Render entity for PointCloudOctree3d not found");
                    (render_entity.id(), nodes)
                })
                .collect(),
        };
        commands
            .entity(render_entity)
            .insert(render_visible_point_cloud_octree_3d_nodes);
    }
}

fn extract_camera_phases(
    mut pointcloud3d_phases: ResMut<ViewBinnedRenderPhases<PointCloudOctree3dPhase>>,
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
        live_entities.insert(retained_view_entity);
    }

    // Clear out all dead views.
    pointcloud3d_phases.retain(|camera_entity, _| live_entities.contains(camera_entity));
}

fn queue_attribute_pass(
    draw_pointcloud_functions: Res<DrawFunctions<PointCloudOctree3dPhase>>,
    mut pipelines: ResMut<SpecializedRenderPipelines<PointCloudOctreePipeline>>,
    pipeline_cache: Res<PipelineCache>,
    point_cloud_octree_pipeline: Res<PointCloudOctreePipeline>,
    render_pointcloud_octrees: Res<RenderOctrees<RenderPointCloudNodeData>>,
    point_cloud_octrees_3d: Query<&PointCloudOctree3d>,
    mut custom_render_phases: ResMut<ViewBinnedRenderPhases<PointCloudOctree3dPhase>>,
    mut views: Query<(&ExtractedView, &RenderVisiblePointCloudOctree3dNodes, &Msaa)>,
    main_entities: Query<&MainEntity>,
    mut next_tick: Local<Tick>,
) {
    for (view, visible_entities, msaa) in &mut views {
        let Some(custom_phase) = custom_render_phases.get_mut(&view.retained_view_entity) else {
            warn!("No custom phase");
            continue;
        };
        let draw_custom = draw_pointcloud_functions.read().id::<DrawPointCloudOctree>();

        let view_key = PointCloudOctreePipelineKey {
            samples: msaa.samples(),
            use_edl: false,
            edl_neighbour_count: 0,
            // Create the key based on the view.
            // In this case we only care about MSAA and HDR
            mesh_pipeline_key: MeshPipelineKey::from_msaa_samples(msaa.samples())
                | MeshPipelineKey::from_hdr(view.hdr),
        };

        let pipeline_id = pipelines.specialize(
            &pipeline_cache,
            &point_cloud_octree_pipeline,
            view_key.clone(),
        );

        // Since our phase can work on any 3d mesh we can reuse the default mesh 3d filter
        for (entity, node_ids) in visible_entities.octrees.iter() {
            let Ok(main_entity) = main_entities.get(*entity) else {
                warn!("Render entity not found, skipping.");
                continue;
            };
            let Ok(point_cloud_octree_3d) = point_cloud_octrees_3d.get(*entity) else {
                warn!("point_cloud_octree_3d missing");
                continue;
            };
            let Some(render_pointcloud_octree) =
                render_pointcloud_octrees.get(point_cloud_octree_3d)
            else {
                warn!("render_pointcloud_octree missing");
                continue;
            };

            // Bump the change tick in order to force Bevy to rebuild the bin.
            let this_tick = next_tick.get() + 1;
            next_tick.set(this_tick);

            for node_id in node_ids {
                let Some(render_node) = render_pointcloud_octree.nodes.get(node_id) else {
                    warn!("missing render node");
                    continue;
                };

                // At this point we have all the data we need to create a phase item and add it to our
                // phase
                // custom_phase.add(PointCloud3dPhase {
                //     // Sort the data based on the distance to the view
                //     // sort_key: FloatOrd(distance),
                //     // entity: (*render_entity, *visible_entity),
                //     // pipeline: pipeline_id,
                //     // draw_function: draw_custom,
                //     // // Sorted phase items aren't batched
                //     // batch_range: 0..1,
                //     // extra_index: PhaseItemExtraIndex::None,
                //     // indexed: mesh.indexed(),
                //     batch_set_key: PointCloud3dBatchSetKey {
                //         pipeline: pipeline_id,
                //         draw_function: draw_custom,
                //     },
                //     bin_key: Pointcloud3dBinKey {
                //         asset_id: point_cloud_3d.0.id(),
                //         node_id: *node_id,
                //     },
                //     representative_entity: (*entity, *main_entity),
                //     batch_range: Default::default(),
                //     extra_index: PhaseItemExtraIndex::None,
                // });

                // info!("adding custom render phrase");
                custom_phase.add(
                    PointCloudOctree3dBatchSetKey {
                        pipeline: pipeline_id,
                        draw_function: draw_custom,
                    },
                    PointCloudOctree3dBinKey {
                        asset_id: point_cloud_octree_3d.0.id(),
                        node_id: *node_id,
                    },
                    // (*entity, Entity::PLACEHOLDER.into()),
                    (*entity, *main_entity),
                    InputUniformIndex::default(),
                    BinnedRenderPhaseType::NonMesh,
                    *next_tick,
                );
            }
        }
    }
}
