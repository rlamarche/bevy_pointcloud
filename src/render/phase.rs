use bevy::{
    core_pipeline::core_3d::{Opaque3d, Opaque3dBatchSetKey, Opaque3dBinKey},
    ecs::system::{Query, Res, ResMut},
    log::{info, warn},
    pbr::{MeshPipelineViewLayoutKey, ViewKeyCache},
    render::{
        camera::DirtySpecializations,
        mesh::{allocator::MeshSlabs, RenderMesh},
        render_asset::RenderAssets,
        render_phase::{
            BinnedRenderPhaseType, DrawFunctions, InputUniformIndex, ViewBinnedRenderPhases,
        },
        render_resource::{PipelineCache, SpecializedRenderPipelines},
        view::{ExtractedView, RenderVisibleEntities},
    },
};

use crate::{
    DrawPointCloud, PendingPointCloudPhaseItemQueues, PointCloudChunk3d, PointCloudPipeline,
    ShapeMeshes, RenderPointCloudChunk, RenderVisiblePointCloudEntities,
};

#[expect(
    clippy::too_many_arguments,
    reason = "it's a system that needs a lot of arguments"
)]
pub fn queue_point_clouds(
    mut opaque_render_phases: ResMut<ViewBinnedRenderPhases<Opaque3d>>,
    views: Query<(
        &ExtractedView,
        &RenderVisibleEntities,
        &RenderVisiblePointCloudEntities,
    )>,
    opaque_3d_draw_functions: Res<DrawFunctions<Opaque3d>>,
    mut pipelines: ResMut<SpecializedRenderPipelines<PointCloudPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    point_cloud_pipeline: Res<PointCloudPipeline>,
    view_key_cache: Res<ViewKeyCache>,
    point_clouds: Query<&PointCloudChunk3d>,
    render_chunks: Res<RenderAssets<RenderPointCloudChunk>>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    dirty_specializations: Res<DirtySpecializations>,
    mut pending_point_cloud_phase_item_queues: ResMut<PendingPointCloudPhaseItemQueues>,
    point_meshes: Res<ShapeMeshes>,
) {
    let draw_function = opaque_3d_draw_functions.read().id::<DrawPointCloud>();

    for (view, visible_entities, _render_visible_point_cloud_entities) in &views {
        let Some(opaque_phase) = opaque_render_phases.get_mut(&view.retained_view_entity) else {
            continue;
        };

        // TODO: fill this array
        let Some(render_visible_point_cloud_entities) = visible_entities.get::<PointCloudChunk3d>()
        else {
            warn!("Visible entities not found in queue_point_clouds");
            continue;
        };

        // First, remove meshes that need to be respecialized, and those that were removed, from the
        // bins.
        for &main_entity in dirty_specializations.iter_to_dequeue(
            view.retained_view_entity,
            render_visible_point_cloud_entities,
        ) {
            info!("remove phase");
            opaque_phase.remove(main_entity);
        }

        // Fetch the pending mesh material queues for this view.
        // let view_pending_point_cloud_queues = pending_point_cloud_phase_item_queues
        //     .get_mut(&view.retained_view_entity)
        //     .expect(
        //         "View pending mesh material queues should have been created in \
        //          `specialize_material_meshes`",
        //     );

        let view_pending_point_cloud_queues =
            pending_point_cloud_phase_item_queues.prepare_for_new_frame(view.retained_view_entity);

        let Some(&mesh_pipeline_key) = view_key_cache.get(&view.retained_view_entity) else {
            continue;
        };

        let view_key = MeshPipelineViewLayoutKey::from(mesh_pipeline_key);

        // Now iterate through all newly-visible entities and those needing respecialization.
        for (&render_entity, &main_entity) in dirty_specializations.iter_to_queue(
            view.retained_view_entity,
            render_visible_point_cloud_entities,
            &view_pending_point_cloud_queues.prev_frame,
        ) {
            info!("add phase start");
            let Ok(point_cloud_chunk_3d) = point_clouds.get(render_entity) else {
                warn!("point cloud chunk 3d not found");

                view_pending_point_cloud_queues
                    .current_frame
                    .insert((render_entity, main_entity));
                continue;
            };
            let Some(quad_mesh) = render_meshes.get(&point_meshes.quad_mesh) else {
                warn!("quad mesh not found");

                view_pending_point_cloud_queues
                    .current_frame
                    .insert((render_entity, main_entity));
                continue;
            };
            let Some(RenderPointCloudChunk {
                mesh: Some(points_mesh),
            }) = render_chunks.get(point_cloud_chunk_3d)
            else {
                warn!("RenderPointCloudChunk not found");

                view_pending_point_cloud_queues
                    .current_frame
                    .insert((render_entity, main_entity));
                continue;
            };
            let Some(points_mesh) = render_meshes.get(points_mesh) else {
                warn!("points_mesh {:?} not found", points_mesh);

                view_pending_point_cloud_queues
                    .current_frame
                    .insert((render_entity, main_entity));
                continue;
            };
            warn!("points_mesh found: {:?}", points_mesh);

            let key = (
                quad_mesh.layout.clone(),
                points_mesh.layout.clone(),
                view_key,
            );

            let pipeline_id = pipelines.specialize(&pipeline_cache, &point_cloud_pipeline, key);

            info!("add opaque phase");
            opaque_phase.add(
                Opaque3dBatchSetKey {
                    draw_function,
                    pipeline: pipeline_id,
                    material_bind_group_index: None,
                    lightmap_slab: None,
                    slabs: MeshSlabs::default(),
                },
                Opaque3dBinKey {
                    asset_id: point_cloud_chunk_3d.into(),
                },
                (render_entity, main_entity),
                InputUniformIndex::default(),
                BinnedRenderPhaseType::NonMesh,
            );
        }
    }
}
