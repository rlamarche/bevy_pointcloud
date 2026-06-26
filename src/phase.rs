use bevy::{
    core_pipeline::core_3d::{Opaque3d, Opaque3dBatchSetKey, Opaque3dBinKey},
    ecs::{
        resource::Resource,
        system::{Query, Res, ResMut},
    },
    log::info,
    pbr::{MeshPipelineViewLayoutKey, ViewKeyCache},
    prelude::{Deref, DerefMut},
    render::{
        camera::{DirtySpecializations, PendingQueues},
        mesh::{allocator::MeshSlabs, RenderMesh},
        render_asset::RenderAssets,
        render_phase::{
            BinnedRenderPhaseType, DrawFunctions, InputUniformIndex, ViewBinnedRenderPhases,
        },
        render_resource::{PipelineCache, SpecializedRenderPipelines},
        view::{ExtractedView, RenderVisibleEntities},
    },
};

use crate::{DrawPointCloud, PointCloud, PointCloudPipeline};

/// A resource that holds entities that couldn't be specialized and/or queued
/// yet because their dependent assets haven't loaded yet.
///
/// In this particular example, entities with custom rendering can always be
/// specialized, so this resource goes unused in practice. However, we still
/// need it, because [`DirtySpecializations`] requires such a resource.
///
/// See the documentation of [`PendingQueues`] for more information.
#[derive(Default, Deref, DerefMut, Resource)]
pub struct PendingPointCloudPhaseItemQueues(pub PendingQueues);

#[expect(
    clippy::too_many_arguments,
    reason = "it's a system that needs a lot of arguments"
)]
pub fn queue_point_clouds(
    mut opaque_render_phases: ResMut<ViewBinnedRenderPhases<Opaque3d>>,
    opaque_3d_draw_functions: Res<DrawFunctions<Opaque3d>>,
    mut pipelines: ResMut<SpecializedRenderPipelines<PointCloudPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    point_cloud_pipeline: Res<PointCloudPipeline>,
    views: Query<(&ExtractedView, &RenderVisibleEntities)>,
    view_key_cache: Res<ViewKeyCache>,
    point_clouds: Query<&PointCloud>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    dirty_specializations: Res<DirtySpecializations>,
    mut pending_custom_phase_item_queues: ResMut<PendingPointCloudPhaseItemQueues>,
) {
    let draw_function = opaque_3d_draw_functions.read().id::<DrawPointCloud>();

    for (view, view_visible_entities) in &views {
        let Some(opaque_phase) = opaque_render_phases.get_mut(&view.retained_view_entity) else {
            continue;
        };

        // Fetch the list of visible entities in the `CustomRenderedEntity`
        // class. If there are no such entities, then we have no entities to
        // render, and we're done.
        let Some(render_visible_mesh_entities) = view_visible_entities.get::<PointCloud>() else {
            info!("No render_visible_mesh_entities");
            continue;
        };

        let Some(&mesh_pipeline_key) = view_key_cache.get(&view.retained_view_entity) else {
            continue;
        };

        let view_key = MeshPipelineViewLayoutKey::from(mesh_pipeline_key);

        let view_pending_custom_phase_item_queues =
            pending_custom_phase_item_queues.prepare_for_new_frame(view.retained_view_entity);

        // First, remove meshes that need to be respecialized, and those that
        // were removed, from the bins.
        for &main_entity in dirty_specializations
            .iter_to_dequeue(view.retained_view_entity, render_visible_mesh_entities)
        {
            opaque_phase.remove(main_entity);
        }

        // Find all the custom rendered entities that are visible from this
        // view.
        for (render_entity, main_entity) in dirty_specializations.iter_to_queue(
            view.retained_view_entity,
            render_visible_mesh_entities,
            &view_pending_custom_phase_item_queues.prev_frame,
        ) {
            let Ok(point_cloud) = point_clouds.get(*render_entity) else {
                continue;
            };

            let Some(quad_mesh) = render_meshes.get(&point_cloud.quad_mesh) else {
                continue;
            };
            let Some(points_mesh) = render_meshes.get(&point_cloud.points_mesh) else {
                continue;
            };

            let key = (
                quad_mesh.layout.clone(),
                points_mesh.layout.clone(),
                view_key,
            );

            let pipeline_id = pipelines.specialize(&pipeline_cache, &point_cloud_pipeline, key);

            info!("Add opaque phase");

            opaque_phase.add(
                Opaque3dBatchSetKey {
                    draw_function,
                    pipeline: pipeline_id,
                    material_bind_group_index: None,
                    lightmap_slab: None,
                    slabs: MeshSlabs::default(),
                },
                Opaque3dBinKey {
                    asset_id: point_cloud.points_mesh.id().untyped(),
                },
                (*render_entity, *main_entity),
                InputUniformIndex::default(),
                BinnedRenderPhaseType::NonMesh,
            );
        }
    }
}
