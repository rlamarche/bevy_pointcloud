use bevy::{
    core_pipeline::{
        core_3d::{
            AlphaMask3d, Opaque3d, Opaque3dBatchSetKey, Opaque3dBinKey, Transparent3d,
            TransparentSortingInfo3d,
        },
        prepass::{OpaqueNoLightmap3dBatchSetKey, OpaqueNoLightmap3dBinKey},
    },
    ecs::system::{Query, Res, ResMut},
    log::{info, warn},
    material::{OpaqueRendererMethod, RenderPhaseType},
    pbr::{
        get_mesh_instance_world_from_local, MeshInputUniform, MeshUniform, RenderMeshInstances,
        Transmissive3d,
    },
    render::{
        batching::gpu_preprocessing::BatchedInstanceBuffers,
        erased_render_asset::ErasedRenderAssets,
        mesh::{allocator::MeshAllocator, RenderMesh},
        render_asset::RenderAssets,
        render_phase::{
            BinnedRenderPhaseType, PhaseItemExtraIndex, ViewBinnedRenderPhases,
            ViewSortedRenderPhases,
        },
        view::{ExtractedView, RenderVisibleEntities},
    },
};

use crate::{
    render::{
        BinnedRenderPhaseExt, MainPassAlphaMaskDrawFunction, MainPassOpaqueDrawFunction,
        MainPassTransmissiveDrawFunction, MainPassTransparentDrawFunction,
        PendingPointCloudMaterialQueues, PointCloudDirtySpecializations,
        PreparedPointCloudMaterial, RenderPointCloudChunkInstances,
        RenderPointCloudMaterialInstances, SpecializedPointCloudMaterialPipelineCache,
    },
    PointCloudChunk3d,
};

/// For each view, iterates over all the meshes visible from that view and adds
/// them to [`BinnedRenderPhase`]s or [`SortedRenderPhase`]s as appropriate.
pub fn queue_material_meshes(
    render_materials: Res<ErasedRenderAssets<PreparedPointCloudMaterial>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    render_material_instances: Res<RenderPointCloudMaterialInstances>,
    render_point_cloud_chunk_instances: Res<RenderPointCloudChunkInstances>,
    mesh_assets: Res<RenderAssets<RenderMesh>>,
    mesh_allocator: Res<MeshAllocator>,
    maybe_batched_instance_buffers: Option<
        Res<BatchedInstanceBuffers<MeshUniform, MeshInputUniform>>,
    >,
    mut opaque_render_phases: ResMut<ViewBinnedRenderPhases<Opaque3d>>,
    mut alpha_mask_render_phases: ResMut<ViewBinnedRenderPhases<AlphaMask3d>>,
    mut transmissive_render_phases: ResMut<ViewSortedRenderPhases<Transmissive3d>>,
    mut transparent_render_phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    mut pending_mesh_material_queues: ResMut<PendingPointCloudMaterialQueues>,
    views: Query<(&ExtractedView, &RenderVisibleEntities)>,
    // don't know why it is `ResMut` here, but I suspect it is to have exclusive read access
    specialized_material_pipeline_cache: ResMut<SpecializedPointCloudMaterialPipelineCache>,
    dirty_specializations: Res<PointCloudDirtySpecializations>,
) {
    for (view, visible_entities) in &views {
        let (
            Some(opaque_phase),
            Some(alpha_mask_phase),
            Some(transmissive_phase),
            Some(transparent_phase),
        ) = (
            opaque_render_phases.get_mut(&view.retained_view_entity),
            alpha_mask_render_phases.get_mut(&view.retained_view_entity),
            transmissive_render_phases.get_mut(&view.retained_view_entity),
            transparent_render_phases.get_mut(&view.retained_view_entity),
        )
        else {
            continue;
        };

        // info!("opaque phase items count: {}", opaque_phase.count_items());

        let Some(view_specialized_material_pipeline_cache) =
            specialized_material_pipeline_cache.get(&view.retained_view_entity)
        else {
            continue;
        };

        let Some(visible_entities_class) = visible_entities.get::<PointCloudChunk3d>() else {
            continue;
        };

        // First, remove meshes that need to be respecialized, and those that were removed, from the
        // bins.
        for (render_entity, main_entity) in
            dirty_specializations.iter_to_dequeue(view.retained_view_entity, visible_entities_class)
        {
            let Some(render_point_cloud_chunk_instance) = (match render_point_cloud_chunk_instances
                .previous
                .get(render_entity)
            {
                Some(value) => Some(value),
                None => render_point_cloud_chunk_instances.get(render_entity),
            }) else {
                warn!(
                    "RenderPointCloudChunkInstance not found for entity {:?} when removing phase",
                    main_entity
                );
                continue;
            };

            opaque_phase.remove_unbatchable_entity_pair(
                render_entity,
                &render_point_cloud_chunk_instance.root_entity,
            );
            alpha_mask_phase.remove_unbatchable_entity_pair(
                render_entity,
                &render_point_cloud_chunk_instance.root_entity,
            );
            transmissive_phase.remove(
                *render_entity,
                render_point_cloud_chunk_instance.root_entity,
            );

            transparent_phase.remove(
                *render_entity,
                render_point_cloud_chunk_instance.root_entity,
            );
        }

        // Fetch the pending mesh material queues for this view.
        let view_pending_mesh_material_queues = pending_mesh_material_queues
            .get_mut(&view.retained_view_entity)
            .expect(
                "View pending mesh material queues should have been created in \
                 `specialize_material_meshes`",
            );

        // Now iterate through all newly-visible entities and those needing respecialization.
        for (render_entity, visible_entity) in dirty_specializations.iter_to_queue(
            view.retained_view_entity,
            visible_entities_class,
            &view_pending_mesh_material_queues.prev_frame,
        ) {
            let Some(pipeline_id) = view_specialized_material_pipeline_cache
                .get(render_entity)
                .copied()
            else {
                continue;
            };

            // our entity is a chunk, we need parent point cloud to get its specialized pipeline
            // & material
            let Some(render_point_cloud_chunk_instance) =
                render_point_cloud_chunk_instances.get(render_entity)
            else {
                warn!(
                    "RenderPointCloudChunkInstance not found for entity {:?}",
                    visible_entity
                );
                continue;
            };

            // Check for material instance, mesh, and material. If any of these
            // fail, it's probably because the relevant asset hasn't loaded yet.
            // In that case, add the entity to the list of pending mesh
            // materials and bail.
            let Some(material_instance) = render_material_instances
                .instances
                .get(&render_point_cloud_chunk_instance.root_entity)
            else {
                warn!("material instance not ready, queue");
                view_pending_mesh_material_queues
                    .current_frame
                    .insert((*render_entity, *visible_entity));
                continue;
            };
            // get the mesh instance from the root entity
            let Some(mesh_instance) = render_mesh_instances
                .render_mesh_queue_data(render_point_cloud_chunk_instance.root_entity)
            else {
                warn!("mesh instance not ready, queue");
                view_pending_mesh_material_queues
                    .current_frame
                    .insert((*render_entity, *visible_entity));
                continue;
            };
            let Some(material) = render_materials.get(material_instance.asset_id) else {
                warn!("material not ready, queue");
                view_pending_mesh_material_queues
                    .current_frame
                    .insert((*render_entity, *visible_entity));
                continue;
            };

            // Fetch the slabs that this mesh resides in.
            let Some(mesh_slabs) =
                mesh_allocator.mesh_slabs(&render_point_cloud_chunk_instance.mesh_asset_id)
            else {
                warn!(
                    "mesh slab not found for visible entity {:?}",
                    visible_entity
                );
                view_pending_mesh_material_queues
                    .current_frame
                    .insert((*render_entity, *visible_entity));
                continue;
            };

            match material.properties.render_phase_type {
                RenderPhaseType::Transmissive => {
                    let Some(draw_function) = material
                        .properties
                        .get_draw_function(MainPassTransmissiveDrawFunction)
                    else {
                        continue;
                    };
                    transmissive_phase.add_retained(Transmissive3d {
                        sorting_info: TransparentSortingInfo3d::Sorted {
                            mesh_center: get_mesh_instance_world_from_local(
                                *visible_entity,
                                mesh_instance.current_uniform_index,
                                &render_mesh_instances,
                                maybe_batched_instance_buffers.as_deref(),
                            )
                            .transform_point3(
                                mesh_assets
                                    .get(mesh_instance.mesh_asset_id())
                                    .unwrap()
                                    .aabb_center,
                            ),
                            depth_bias: material.properties.depth_bias,
                        },
                        entity: (
                            // Bevy sends PLACEHOLDER here, why ?
                            *render_entity,
                            // use the root entity here to correctly handle `RenderMeshInstances`
                            // in binned render phases
                            render_point_cloud_chunk_instance.root_entity,
                        ),
                        draw_function,
                        pipeline: pipeline_id,
                        batch_range: 0..1,
                        extra_index: PhaseItemExtraIndex::None,
                        indexed: mesh_slabs.index_slab_id.is_some(),
                        // Filled in later.
                        distance: 0.0,
                    });
                }
                RenderPhaseType::Opaque => {
                    if material.properties.render_method == OpaqueRendererMethod::Deferred {
                        // Even though we aren't going to insert the entity into
                        // a bin, we still want to update its cache entry. That
                        // way, we know we don't need to re-examine it in future
                        // frames.
                        opaque_phase.update_cache(*visible_entity, None);
                        continue;
                    }
                    let Some(draw_function) = material
                        .properties
                        .get_draw_function(MainPassOpaqueDrawFunction)
                    else {
                        warn!("draw function not found");
                        continue;
                    };
                    let batch_set_key = Opaque3dBatchSetKey {
                        pipeline: pipeline_id,
                        draw_function,
                        material_bind_group_index: Some(material.binding.group.0),
                        slabs: mesh_slabs,
                        lightmap_slab: mesh_instance
                            .shared
                            .lightmap_slab_index()
                            .map(|index| *index),
                    };
                    let bin_key = Opaque3dBinKey {
                        asset_id: render_point_cloud_chunk_instance.mesh_asset_id.into(),
                    };

                    opaque_phase.add(
                        batch_set_key,
                        bin_key,
                        (
                            *render_entity,
                            // use the root entity here to correctly handle
                            // [`GetFullBatchData::get_binned_index`]
                            // in binned render phases
                            render_point_cloud_chunk_instance.root_entity,
                        ),
                        mesh_instance.current_uniform_index,
                        BinnedRenderPhaseType::UnbatchableMesh,
                    );
                }
                // Alpha mask
                RenderPhaseType::AlphaMask => {
                    let Some(draw_function) = material
                        .properties
                        .get_draw_function(MainPassAlphaMaskDrawFunction)
                    else {
                        info!("missing drawfunction for MainPassAlphaMaskDrawFunction");
                        continue;
                    };
                    let batch_set_key = OpaqueNoLightmap3dBatchSetKey {
                        draw_function,
                        pipeline: pipeline_id,
                        material_bind_group_index: Some(material.binding.group.0),
                        slabs: mesh_slabs,
                    };
                    let bin_key = OpaqueNoLightmap3dBinKey {
                        asset_id: mesh_instance.mesh_asset_id().into(),
                    };
                    alpha_mask_phase.add(
                        batch_set_key,
                        bin_key,
                        (
                            *render_entity,
                            // use the root entity here to correctly handle
                            // [`GetFullBatchData::get_binned_index`]
                            // in binned render phases
                            render_point_cloud_chunk_instance.root_entity,
                        ),
                        mesh_instance.current_uniform_index,
                        BinnedRenderPhaseType::UnbatchableMesh,
                    );
                }
                RenderPhaseType::Transparent => {
                    let Some(draw_function) = material
                        .properties
                        .get_draw_function(MainPassTransparentDrawFunction)
                    else {
                        continue;
                    };

                    transparent_phase.add_retained(Transparent3d {
                        sorting_info: TransparentSortingInfo3d::Sorted {
                            mesh_center: get_mesh_instance_world_from_local(
                                render_point_cloud_chunk_instance.root_entity,
                                mesh_instance.current_uniform_index,
                                &render_mesh_instances,
                                maybe_batched_instance_buffers.as_deref(),
                            )
                            .transform_point3(
                                mesh_assets
                                    .get(render_point_cloud_chunk_instance.mesh_asset_id)
                                    .unwrap()
                                    .aabb_center,
                            ),
                            depth_bias: material.properties.depth_bias,
                        },
                        entity: (
                            // Bevy sends PLACEHOLDER here, why ?
                            *render_entity,
                            // use the root entity here to correctly handle
                            // [`GetFullBatchData::get_binned_index`]
                            // in binned render phases
                            render_point_cloud_chunk_instance.root_entity,
                        ),
                        draw_function,
                        pipeline: pipeline_id,
                        batch_range: 0..1,
                        extra_index: PhaseItemExtraIndex::None,
                        indexed: mesh_slabs.index_slab_id.is_some(),
                        // Filled in later.
                        distance: 0.0,
                    });
                }
            }
        }
    }
}
