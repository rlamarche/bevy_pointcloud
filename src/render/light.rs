use std::{any::TypeId, sync::Arc};

use bevy::{
    camera::visibility::RenderLayers,
    ecs::{
        entity::{Entity, EntityHashMap},
        resource::Resource,
        system::{Local, Query, Res, ResMut, SystemParam, SystemState},
        world::World,
    },
    log::{debug, error, warn},
    material::{
        descriptor::CachedRenderPipelineId, key::ErasedMeshPipelineKey, labels::DrawFunctionId,
        AlphaMode,
    },
    mesh::MeshVertexBufferLayoutRef,
    pbr::{
        LightEntity, LightKeyCache, MeshPipelineKey, PrepassPipeline, RenderLightmaps,
        RenderMeshInstanceFlags, RenderMeshInstances, Shadow, ShadowBatchSetKey, ShadowBinKey,
    },
    platform::{
        collections::{HashMap, HashSet},
        hash::FixedHasher,
    },
    prelude::{Deref, DerefMut},
    render::{
        batching::gpu_preprocessing::GpuPreprocessingSupport,
        camera::PendingQueues,
        erased_render_asset::ErasedRenderAssets,
        mesh::{allocator::MeshAllocator, RenderMesh},
        render_asset::RenderAssets,
        render_phase::{BinnedRenderPhaseType, ViewBinnedRenderPhases},
        sync_world::MainEntity,
        view::{
            ExtractedView, RenderShadowMapVisibleEntities, RenderVisibleEntities,
            RetainedViewEntity,
        },
    },
};

use crate::{
    BinnedRenderPhaseExt, ErasedMaterialPipelineKey, ErasedSplatPipelineKey, MaterialProperties,
    PointCloudChunk3d, PointCloudDirtySpecializations, PointCloudTopologyKind, PreparedMaterial,
    RenderPointCloudChunkInstances, RenderPointCloudInstances, RenderPointCloudMaterialInstances,
    ShadowsDepthOnlyDrawFunction, ShadowsDrawFunction, SplatPipelineKey,
};

pub(crate) struct ShadowSpecializationWorkItem {
    render_entity: Entity,
    // visible_entity: MainEntity,
    retained_view_entity: RetainedViewEntity,
    mesh_key: MeshPipelineKey,
    splat_key: SplatPipelineKey,
    splat_layout: MeshVertexBufferLayoutRef,
    instance_layout: MeshVertexBufferLayoutRef,
    properties: Arc<MaterialProperties>,
    material_type_id: TypeId,
}

#[derive(Resource, Deref, DerefMut, Default)]
pub struct SpecializedShadowMaterialPipelineCache {
    // view light entity -> view pipeline cache
    #[deref]
    map: HashMap<RetainedViewEntity, SpecializedShadowMaterialViewPipelineCache>,
}

#[derive(Deref, DerefMut, Default)]
pub struct SpecializedShadowMaterialViewPipelineCache {
    #[deref]
    map: EntityHashMap<(CachedRenderPipelineId, DrawFunctionId)>,
}

/// Holds all entities with mesh materials for which the shadow pass couldn't be
/// specialized and/or queued because their materials hadn't loaded yet.
///
/// See the [`PendingQueues`] documentation for more information.
#[derive(Default, Deref, DerefMut, Resource)]
pub struct PendingShadowQueues(pub PendingQueues);

#[derive(SystemParam)]
pub(crate) struct SpecializeShadowsSystemParam<'w, 's> {
    render_meshes: Res<'w, RenderAssets<RenderMesh>>,
    render_mesh_instances: Res<'w, RenderMeshInstances>,
    render_materials: Res<'w, ErasedRenderAssets<PreparedMaterial>>,
    render_material_instances: Res<'w, RenderPointCloudMaterialInstances>,
    render_point_cloud_instances: Res<'w, RenderPointCloudInstances>,
    render_point_cloud_chunk_instances: Res<'w, RenderPointCloudChunkInstances>,
    shadow_render_phases: Res<'w, ViewBinnedRenderPhases<Shadow>>,
    render_lightmaps: Res<'w, RenderLightmaps>,
    view_light_entities: Query<'w, 's, (&'static LightEntity, &'static ExtractedView)>,
    shadow_map_visible_entities_query: Query<'w, 's, &'static RenderShadowMapVisibleEntities>,
    light_key_cache: Res<'w, LightKeyCache>,
    specialized_shadow_material_pipeline_cache: ResMut<'w, SpecializedShadowMaterialPipelineCache>,
    pending_shadow_queues: ResMut<'w, PendingShadowQueues>,
    dirty_specializations: Res<'w, PointCloudDirtySpecializations>,
}

pub(crate) fn specialize_shadows(
    world: &mut World,
    state: &mut SystemState<SpecializeShadowsSystemParam>,
    mut work_items: Local<Vec<ShadowSpecializationWorkItem>>,
    mut all_shadow_views: Local<HashSet<RetainedViewEntity, FixedHasher>>,
) {
    work_items.clear();
    all_shadow_views.clear();

    {
        let SpecializeShadowsSystemParam {
            render_meshes,
            render_mesh_instances,
            render_materials,
            render_material_instances,
            render_point_cloud_instances,
            render_point_cloud_chunk_instances,
            shadow_render_phases,
            render_lightmaps: _render_lightmaps,
            view_light_entities,
            shadow_map_visible_entities_query,
            light_key_cache,
            mut specialized_shadow_material_pipeline_cache,
            mut pending_shadow_queues,
            dirty_specializations,
        } = state.get_mut(world).unwrap();

        for (light_entity, extracted_view_light) in &view_light_entities {
            all_shadow_views.insert(extracted_view_light.retained_view_entity);

            if !shadow_render_phases.contains_key(&extracted_view_light.retained_view_entity) {
                continue;
            }
            let Some(light_key) = light_key_cache.get(&extracted_view_light.retained_view_entity)
            else {
                continue;
            };

            let visible_entities = get_shadow_map_visible_entities(
                &shadow_map_visible_entities_query,
                light_entity,
                extracted_view_light,
            );

            let mut maybe_specialized_shadow_material_pipeline_cache =
                specialized_shadow_material_pipeline_cache
                    .get_mut(&extracted_view_light.retained_view_entity);

            // Remove cached pipeline IDs corresponding to entities that
            // either have been removed or need to be respecialized.
            if let Some(ref mut specialized_shadow_material_pipeline_cache) =
                maybe_specialized_shadow_material_pipeline_cache
            {
                if dirty_specializations
                    .must_wipe_specializations_for_view(extracted_view_light.retained_view_entity)
                {
                    specialized_shadow_material_pipeline_cache.clear();
                } else {
                    for (&renderable_entity, &_main_entity) in
                        dirty_specializations.iter_to_despecialize()
                    {
                        specialized_shadow_material_pipeline_cache.remove(&renderable_entity);
                    }
                }
            }

            // Initialize the pending queues.
            let view_pending_shadow_queues = pending_shadow_queues
                .prepare_for_new_frame(extracted_view_light.retained_view_entity);

            // NOTE: Lights with shadow mapping disabled will have no visible entities
            // so no meshes will be queued

            let Some(visible_entities_class) = visible_entities.get::<PointCloudChunk3d>() else {
                continue;
            };

            // Now process all shadow meshes that need to be re-specialized.
            for (render_entity, visible_entity) in dirty_specializations.iter_to_specialize(
                extracted_view_light.retained_view_entity,
                visible_entities_class,
                &view_pending_shadow_queues.prev_frame,
            ) {
                if maybe_specialized_shadow_material_pipeline_cache
                    .as_ref()
                    .is_some_and(|specialized_shadow_material_pipeline_cache| {
                        specialized_shadow_material_pipeline_cache.contains_key(render_entity)
                    })
                {
                    continue;
                }

                // our entity is a chunk, we need parent point cloud to get its specialized pipeline
                // & material
                let Some(render_point_cloud_chunk_instance) =
                    render_point_cloud_chunk_instances.get(render_entity)
                else {
                    warn!(
                        "RenderPointCloudChunkInstance not found for entity {:?} in shadows",
                        visible_entity
                    );
                    continue;
                };

                let Some(render_point_cloud_instance) = render_point_cloud_instances
                    .get(&render_point_cloud_chunk_instance.root_entity)
                else {
                    warn!(
                        "RenderPointCloudInstance not found for entity {:?} in shadows",
                        visible_entity
                    );
                    continue;
                };

                // Check for material instance, mesh, and material. If any of
                // these fail, it's probably because the relevant asset hasn't
                // loaded yet. In that case, add the entity to the list of
                // pending mesh materials and bail.
                let Some(material_instance) = render_material_instances
                    .instances
                    .get(&render_point_cloud_chunk_instance.root_entity)
                else {
                    warn!(
                        "Unable to load material instance for entity {:?} in shadows",
                        visible_entity
                    );
                    view_pending_shadow_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };
                // get the mesh instance from the root entity
                let Some(mesh_instance) = render_mesh_instances
                    .render_mesh_queue_data(render_point_cloud_chunk_instance.root_entity)
                else {
                    warn!(
                        "mesh_instance not found for entity {:?} in shadows",
                        render_point_cloud_chunk_instance.root_entity
                    );
                    view_pending_shadow_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };

                // get the mesh from the chunk
                let Some(mesh) = render_meshes.get(render_point_cloud_chunk_instance.mesh_asset_id)
                else {
                    warn!(
                        "render_meshes not found for asset id {:?} in shadows",
                        render_point_cloud_chunk_instance.mesh_asset_id
                    );
                    view_pending_shadow_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };

                let Some(material) = render_materials.get(material_instance.asset_id) else {
                    warn!("render_materials not found in shadows");
                    view_pending_shadow_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };

                if !material.properties.shadows_enabled {
                    // If the material is not a shadow caster, we don't need to specialize it.
                    continue;
                }

                if !mesh_instance
                    .flags()
                    .contains(RenderMeshInstanceFlags::SHADOW_CASTER)
                {
                    continue;
                }

                let mut mesh_key =
                    *light_key | MeshPipelineKey::from_bits_retain(mesh.key_bits.bits());

                // Even though we don't use the lightmap in the shadow map, the
                // `SetMeshBindGroup` render command will bind the data for it. So
                // we need to include the appropriate flag in the mesh pipeline key
                // to ensure that the necessary bind group layout entries are
                // present.
                // TODO migrate this ? ([`render_lightmaps.render_lightmaps`] is private)
                // if render_lightmaps
                //     .render_lightmaps
                //     .contains_key(visible_entity)
                // {
                //     mesh_key |= MeshPipelineKey::LIGHTMAPPED;
                // }

                mesh_key |= match material.properties.alpha_mode {
                    AlphaMode::Mask(_)
                    | AlphaMode::Blend
                    | AlphaMode::Premultiplied
                    | AlphaMode::Add
                    | AlphaMode::AlphaToCoverage => MeshPipelineKey::MAY_DISCARD,
                    _ => MeshPipelineKey::NONE,
                };

                let Some(splat_mesh) = render_meshes.get(render_point_cloud_instance.splat) else {
                    warn!("splat mesh not found");
                    view_pending_shadow_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };

                let mut splat_key = (&render_point_cloud_instance.splat_settings).into();

                if matches!(
                    render_point_cloud_instance.topology,
                    PointCloudTopologyKind::Octree
                ) {
                    splat_key |= SplatPipelineKey::IS_OCTREE;
                }

                work_items.push(ShadowSpecializationWorkItem {
                    render_entity: *render_entity,
                    // visible_entity: *visible_entity,
                    retained_view_entity: extracted_view_light.retained_view_entity,
                    mesh_key,
                    splat_key,
                    splat_layout: splat_mesh.layout.clone(),
                    instance_layout: mesh.layout.clone(),
                    properties: material.properties.clone(),
                    material_type_id: material_instance.asset_id.type_id(),
                });
            }
        }

        pending_shadow_queues.expire_stale_views(&all_shadow_views);
    }

    let depth_clip_control_supported = world
        .resource::<PrepassPipeline>()
        .depth_clip_control_supported;

    for item in work_items.drain(..) {
        let Some(prepass_specialize) = item.properties.prepass_specialize else {
            continue;
        };

        let key = ErasedMaterialPipelineKey {
            type_id: item.material_type_id,
            mesh_key: ErasedMeshPipelineKey::new(item.mesh_key),
            splat_key: ErasedSplatPipelineKey::new(item.splat_key),
            material_key: item.properties.material_key.clone(),
        };

        let emulate_unclipped_depth = item
            .mesh_key
            .contains(MeshPipelineKey::UNCLIPPED_DEPTH_ORTHO)
            && !depth_clip_control_supported;
        let is_depth_only_opaque = !item
            .mesh_key
            .intersects(MeshPipelineKey::MAY_DISCARD | MeshPipelineKey::PREPASS_READS_MATERIAL)
            && !emulate_unclipped_depth;
        let draw_function = if is_depth_only_opaque {
            item.properties
                .get_draw_function(ShadowsDepthOnlyDrawFunction)
        } else {
            item.properties.get_draw_function(ShadowsDrawFunction)
        };

        let Some(draw_function) = draw_function else {
            continue;
        };

        match prepass_specialize(
            world,
            key,
            &item.splat_layout,
            &item.instance_layout,
            &item.properties,
        ) {
            Ok(pipeline_id) => {
                world
                    .resource_mut::<SpecializedShadowMaterialPipelineCache>()
                    .entry(item.retained_view_entity)
                    .or_default()
                    .insert(item.render_entity, (pipeline_id, draw_function));
            }
            Err(err) => error!("{}", err),
        }
    }

    // Delete specialized pipelines belonging to views that have expired.
    world
        .resource_mut::<SpecializedShadowMaterialPipelineCache>()
        .retain(|view, _| all_shadow_views.contains(view));
}

/// For each shadow cascade, iterates over all the meshes "visible" from it and
/// adds them to [`BinnedRenderPhase`]s or [`SortedRenderPhase`]s as
/// appropriate.
pub fn queue_shadows(
    render_mesh_instances: Res<RenderMeshInstances>,
    render_materials: Res<ErasedRenderAssets<PreparedMaterial>>,
    render_material_instances: Res<RenderPointCloudMaterialInstances>,
    render_point_cloud_chunk_instances: Res<RenderPointCloudChunkInstances>,
    mut shadow_render_phases: ResMut<ViewBinnedRenderPhases<Shadow>>,
    _gpu_preprocessing_support: Res<GpuPreprocessingSupport>,
    mesh_allocator: Res<MeshAllocator>,
    view_light_entities: Query<(&LightEntity, &ExtractedView, Option<&RenderLayers>)>,
    shadow_map_visible_entities_query: Query<&RenderShadowMapVisibleEntities>,
    specialized_material_pipeline_cache: Res<SpecializedShadowMaterialPipelineCache>,
    mut pending_shadow_queues: ResMut<PendingShadowQueues>,
    dirty_specializations: Res<PointCloudDirtySpecializations>,
) {
    for (light_entity, extracted_view_light, maybe_view_render_layers) in &view_light_entities {
        let Some(shadow_phase) =
            shadow_render_phases.get_mut(&extracted_view_light.retained_view_entity)
        else {
            continue;
        };

        let Some(view_specialized_material_pipeline_cache) =
            specialized_material_pipeline_cache.get(&extracted_view_light.retained_view_entity)
        else {
            continue;
        };

        // Fetch the pending mesh material queues for this view.
        let view_pending_shadow_queues = pending_shadow_queues
            .get_mut(&extracted_view_light.retained_view_entity)
            .expect("View pending shadow queues should have been created in `specialize_shadows`");

        let visible_entities = get_shadow_map_visible_entities(
            &shadow_map_visible_entities_query,
            light_entity,
            extracted_view_light,
        );

        let Some(visible_entities_class) = visible_entities.get::<PointCloudChunk3d>() else {
            continue;
        };

        // First, remove meshes that need to be respecialized, and those that were removed, from the
        // bins.
        for (render_entity, main_entity) in dirty_specializations.iter_to_dequeue(
            extracted_view_light.retained_view_entity,
            visible_entities_class,
        ) {
            let Some(render_point_cloud_chunk_instance) = (match render_point_cloud_chunk_instances
                .previous
                .get(render_entity)
            {
                Some(value) => Some(value),
                None => render_point_cloud_chunk_instances.get(render_entity),
            }) else {
                warn!(
                    "RenderPointCloudChunkInstance not found for entity {:?} when removing shadow phase",
                    main_entity
                );
                continue;
            };

            debug!(
                "remove shadow phase {:?}/{:?}",
                render_point_cloud_chunk_instance.root_entity, render_entity
            );
            shadow_phase.remove_unbatchable_entity_pair(
                render_entity,
                &render_point_cloud_chunk_instance.root_entity,
            );
        }

        // Now iterate through all newly-visible entities and those needing respecialization.
        for (render_entity, visible_entity) in dirty_specializations.iter_to_queue(
            extracted_view_light.retained_view_entity,
            visible_entities_class,
            &view_pending_shadow_queues.prev_frame,
        ) {
            let Some(&(pipeline_id, draw_function)) =
                view_specialized_material_pipeline_cache.get(render_entity)
            else {
                debug!(
                    "view_specialized_material_pipeline_cache not found for entity {:?}",
                    visible_entity
                );
                continue;
            };

            // our entity is a chunk, we need parent point cloud to get its specialized pipeline
            // & material
            let Some(render_point_cloud_chunk_instance) =
                render_point_cloud_chunk_instances.get(render_entity)
            else {
                continue;
            };

            let Some(mesh_instance) = render_mesh_instances
                .render_mesh_queue_data(render_point_cloud_chunk_instance.root_entity)
            else {
                // We couldn't fetch the mesh, probably because it hasn't
                // loaded yet. Add the entity to the list of pending shadows
                // and bail.
                view_pending_shadow_queues
                    .current_frame
                    .insert((*render_entity, *visible_entity));

                warn!("Mesh instance not found {:?}", visible_entity);
                continue;
            };
            if !mesh_instance
                .flags()
                .contains(RenderMeshInstanceFlags::SHADOW_CASTER)
            {
                continue;
            }

            let mesh_layers = mesh_instance.render_layers.as_ref().unwrap_or_default();
            let view_render_layers = maybe_view_render_layers.unwrap_or_default();
            if !view_render_layers.intersects(mesh_layers) {
                continue;
            }

            // get the material from the root entity
            let Some(material_instance) = render_material_instances
                .instances
                .get(&render_point_cloud_chunk_instance.root_entity)
            else {
                // TODO: Bevy is not doing this in its `queue_shadows` system, should I do it ?
                view_pending_shadow_queues
                    .current_frame
                    .insert((*render_entity, *visible_entity));
                continue;
            };

            let Some(material) = render_materials.get(material_instance.asset_id) else {
                // We couldn't fetch the material, probably because the
                // material hasn't been loaded yet. Add the entity to the
                // list of pending shadows and bail.
                view_pending_shadow_queues
                    .current_frame
                    .insert((*render_entity, *visible_entity));
                continue;
            };

            let depth_only_draw_function = material
                .properties
                .get_draw_function(ShadowsDepthOnlyDrawFunction);
            let material_bind_group_index = if Some(draw_function) == depth_only_draw_function {
                None
            } else {
                Some(material.binding.group.0)
            };

            let Some(mesh_slabs) =
                mesh_allocator.mesh_slabs(&render_point_cloud_chunk_instance.mesh_asset_id)
            else {
                warn!(
                    "mesh slab not found for visible entity {:?} in shadows",
                    visible_entity
                );
                view_pending_shadow_queues
                    .current_frame
                    .insert((*render_entity, *visible_entity));
                continue;
            };

            let batch_set_key = ShadowBatchSetKey {
                pipeline: pipeline_id,
                draw_function,
                material_bind_group_index,
                slabs: mesh_slabs,
            };

            shadow_phase.add(
                batch_set_key,
                ShadowBinKey {
                    asset_id: render_point_cloud_chunk_instance.mesh_asset_id.into(),
                },
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
    }
}

/// Returns the [`RenderShadowMapVisibleEntities`] table corresponding to the
/// given [`LightEntity`].
fn get_shadow_map_visible_entities<'w, 's: 'w>(
    shadow_map_visible_entities_query: &'w Query<'w, 's, &'_ RenderShadowMapVisibleEntities>,
    light_entity: &'_ LightEntity,
    extracted_view_light: &'_ ExtractedView,
) -> &'w RenderVisibleEntities {
    match light_entity {
        LightEntity::Directional { light_entity, .. } => {
            let retained_view_entity = extracted_view_light.retained_view_entity;
            shadow_map_visible_entities_query
                .get(*light_entity)
                .expect("Failed to get directional light visible entities")
                .subviews
                .get(&retained_view_entity)
                .expect("Failed to get directional light visible entities for cascade")
        }
        LightEntity::Point {
            light_entity,
            face_index,
        } => {
            // We replace the auxiliary entity with `PLACEHOLDER`
            // because all cubemap views for a single point light
            // currently share the same set of visible entities.
            let retained_view_entity = RetainedViewEntity {
                main_entity: extracted_view_light.retained_view_entity.main_entity,
                auxiliary_entity: MainEntity::from(Entity::PLACEHOLDER),
                subview_index: *face_index as u32,
            };
            shadow_map_visible_entities_query
                .get(*light_entity)
                .expect("Failed to get point light visible entities")
                .subviews
                .get(&retained_view_entity)
                .expect("Failed to get point light visible entity for face")
        }
        LightEntity::Spot { light_entity } => {
            // We replace the auxiliary entity with `PLACEHOLDER`
            // because all shadow maps for a single spot light
            // currently share the same set of visible entities.
            let retained_view_entity = RetainedViewEntity {
                main_entity: extracted_view_light.retained_view_entity.main_entity,
                auxiliary_entity: MainEntity::from(Entity::PLACEHOLDER),
                subview_index: 0,
            };
            shadow_map_visible_entities_query
                .get(*light_entity)
                .expect("Failed to get spot light visible entities")
                .subviews
                .get(&retained_view_entity)
                .expect("Failed to get spot light visible entity for view")
        }
    }
}
