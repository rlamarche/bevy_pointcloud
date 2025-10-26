mod draw;
mod node;
mod phase;
mod pipeline;

use crate::pointcloud_octree::render::draw::DrawPointcloud3d;
use crate::pointcloud_octree::render::node::{Pointcloud3dDrawLabel, Pointcloud3dDrawNode};
use crate::pointcloud_octree::render::phase::Pointcloud3d;
use crate::pointcloud_octree::render::pipeline::PointcloudOctreePipeline;
use bevy_app::prelude::*;
use bevy_asset::prelude::*;
use bevy_asset::{load_internal_asset, uuid_handle};
use bevy_camera::{Camera, Camera3d};
use bevy_core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy_ecs::prelude::*;
use bevy_pbr::MeshPipelineKey;
use bevy_platform::collections::HashSet;
use bevy_render::{Extract, RenderApp};
use bevy_render::batching::gpu_preprocessing::{GpuPreprocessingMode, GpuPreprocessingSupport};
use bevy_render::prelude::*;
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_graph::{RenderGraphExt, ViewNodeRunner};
use bevy_render::render_phase::{AddRenderCommand, DrawFunctions, ViewBinnedRenderPhases};
use bevy_render::render_resource::{PipelineCache, SpecializedRenderPipelines};
use bevy_render::view::{ExtractedView, NoIndirectDrawing, RenderVisibleEntities, RetainedViewEntity};
use bevy_shader::prelude::*;
use crate::octree::extract::RenderOctrees;
use crate::pointcloud_octree::extract::RenderPointCloudNodeData;

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

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<DrawFunctions<Pointcloud3d>>()
            .add_render_command::<Pointcloud3d, DrawPointcloud3d>()
            .init_resource::<ViewBinnedRenderPhases<Pointcloud3d>>()
            .init_resource::<SpecializedRenderPipelines<PointcloudOctreePipeline>>()
            .add_render_graph_node::<ViewNodeRunner<Pointcloud3dDrawNode>>(
                Core3d,
                Pointcloud3dDrawLabel,
            )
            .add_render_graph_edges(Core3d, (Pointcloud3dDrawLabel, Node3d::MainTransparentPass));
    }

    fn finish(&self, app: &mut App) {}
}

fn extract_camera_phases(
    mut pointcloud3d_phases: ResMut<ViewBinnedRenderPhases<Pointcloud3d>>,
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
    draw_pointcloud_functions: Res<DrawFunctions<Pointcloud3d>>,
    mut pipelines: ResMut<SpecializedRenderPipelines<PointcloudOctreePipeline>>,
    pipeline_cache: Res<PipelineCache>,
    pointcloud_octree_pipeline: Res<PointcloudOctreePipeline>,
    render_meshes: Res<RenderOctrees<RenderPointCloudNodeData>>,
    // render_mesh_instances: Res<RenderMeshInstances>,
    mut custom_render_phases: ResMut<ViewBinnedRenderPhases<Pointcloud3d>>,
    mut views: Query<(&ExtractedView, &RenderVisibleEntities, &Msaa)>,
) {
    for (view, visible_entities, msaa) in &mut views {
        let Some(custom_phase) = custom_render_phases.get_mut(&view.retained_view_entity) else {
            continue;
        };
        let draw_custom = draw_pointcloud_functions.read().id::<DrawPointcloud3d>();

        // Create the key based on the view.
        // In this case we only care about MSAA and HDR
        let view_key = MeshPipelineKey::from_msaa_samples(msaa.samples())
            | MeshPipelineKey::from_hdr(view.hdr);

        let rangefinder = view.rangefinder3d();
        // Since our phase can work on any 3d mesh we can reuse the default mesh 3d filter
        for (render_entity, visible_entity) in visible_entities.iter::<Mesh3d>() {
            // We only want meshes with the marker component to be queued to our phase.
            let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(*visible_entity)
            else {
                continue;
            };
            let Some(mesh) = render_meshes.get(mesh_instance.mesh_asset_id) else {
                continue;
            };

            // Specialize the key for the current mesh entity
            // For this example we only specialize based on the mesh topology
            // but you could have more complex keys and that's where you'd need to create those keys
            let mut mesh_key = view_key;
            mesh_key |= MeshPipelineKey::from_primitive_topology(mesh.primitive_topology());

            let pipeline_id = pipelines.specialize(
                &pipeline_cache,
                &pointcloud_octree_pipeline,
                mesh_key,
                &mesh.layout,
            );
            let pipeline_id = match pipeline_id {
                Ok(id) => id,
                Err(err) => {
                    error!("{}", err);
                    continue;
                }
            };
            let distance = rangefinder.distance_translation(&mesh_instance.translation);
            // At this point we have all the data we need to create a phase item and add it to our
            // phase
            custom_phase.add(AttributePass3d {
                // Sort the data based on the distance to the view
                sort_key: FloatOrd(distance),
                entity: (*render_entity, *visible_entity),
                pipeline: pipeline_id,
                draw_function: draw_custom,
                // Sorted phase items aren't batched
                batch_range: 0..1,
                extra_index: PhaseItemExtraIndex::None,
                indexed: mesh.indexed(),
            });
        }
    }
}
