use std::{
    any::{Any, TypeId},
    hash::Hash,
    sync::Arc,
};

use bevy::{
    asset::prelude::AssetChanged,
    core_pipeline::core_3d::{AlphaMask3d, Opaque3d, Transparent3d},
    ecs::{
        entity::{Entity, EntityHashSet},
        query::{Changed, Or, With},
        system::{Local, Query, Res, ResMut, SystemParam, SystemState},
        world::{Mut, World},
    },
    log::{error, warn},
    material::{
        descriptor::{CachedRenderPipelineId, RenderPipelineDescriptor},
        key::ErasedMeshPipelineKey,
        specialize::SpecializedMeshPipelineError,
    },
    mesh::MeshVertexBufferLayoutRef,
    pbr::{
        alpha_mode_pipeline_key, MeshPipelineKey, RenderMeshInstanceFlags, RenderMeshInstances,
        Transmissive3d, ViewKeyCache,
    },
    platform::{collections::HashSet, hash::FixedHasher},
    render::{
        erased_render_asset::ErasedRenderAssets,
        mesh::RenderMesh,
        render_asset::RenderAssets,
        render_phase::{ViewBinnedRenderPhases, ViewSortedRenderPhases},
        render_resource::PipelineCache,
        sync_world::{MainEntity, RenderEntity},
        view::{
            ExtractedView, Msaa, RenderVisibilityRanges, RenderVisibleEntities, RetainedViewEntity,
        },
    },
    utils::Parallel,
};

use crate::{
    render::{
        prepass::{PrepassPipeline, PrepassPipelineSpecializer},
        EntitiesNeedingSpecialization, ErasedMaterialViewSettingsKeys,
        ErasedPointCloudMaterialPipelineKey, ErasedSplatPipelineKey, ErasedViewSettingsKey,
        PendingPointCloudMaterialQueues, PointCloudDirtySpecializations, PointCloudMaterial,
        PointCloudMaterialPipeline, PointCloudMaterialPipelineKey,
        PointCloudMaterialPipelineSpecializer, PointCloudMaterialProperties, PointCloudPipeline,
        PreparedPointCloudMaterial, RenderPointCloudChunkInstances, RenderPointCloudInstances,
        RenderPointCloudMaterialInstances, SpecializedPointCloudMaterialPipelineCache,
        SpecializedPointCloudPipelines, SplatPipelineKey,
    },
    GlobalVisiblePointCloudChunks, PointCloud3d, PointCloudChunk3d, PointCloudMaterial3d,
    PointCloudTopologyKind, SplatSettings,
};

pub fn clear_entities_needing_specialization_removed<M: PointCloudMaterial>(
    mut entities_needing_specialization: ResMut<EntitiesNeedingSpecialization<M>>,
) {
    entities_needing_specialization.removed.clear();
}

/// Finds 3D entities that have changed in such a way as to potentially require
/// specialization and adds them to the [`EntitiesNeedingSpecialization`] list.
pub fn check_entities_needing_specialization<M>(
    needs_specialization: Query<
        Entity,
        (
            Or<(
                Changed<PointCloud3d>,
                // TODO restore when editing hierarchy don't trigger this
                // AssetChanged<PointCloud3d>,
                Changed<PointCloudMaterial3d<M>>,
                Changed<SplatSettings>,
                AssetChanged<PointCloudMaterial3d<M>>,
            )>,
            With<PointCloudMaterial3d<M>>,
        ),
    >,
    chunks_needing_specialization: Query<
        Entity,
        (
            Or<(Changed<PointCloudChunk3d>, AssetChanged<PointCloudChunk3d>)>,
            With<PointCloudChunk3d>,
        ),
    >,
    global_visible_point_cloud_chunks: Res<GlobalVisiblePointCloudChunks>,
    mut par_local: Local<Parallel<Vec<Entity>>>,
    mut entities_needing_specialization: ResMut<EntitiesNeedingSpecialization<M>>,
    // reused hashset to prevent duplicates
    mut deduplicate_entities_hash_set: Local<EntityHashSet>,
) where
    M: PointCloudMaterial,
{
    entities_needing_specialization.changed.clear();
    // `entities_needing_specialization.removed` is cleared in the `First` schedule because it is
    // also populated by observers.

    // When a [`PointCloud3d`] or its material changed, we need to re-specialize all it's
    // children.
    needs_specialization.par_iter().for_each(|entity| {
        // When a [`PointCloud3d`] or its material changed, we need to re-specialize all it's
        // children.
        if let Some(chunks) = global_visible_point_cloud_chunks.get(&entity) {
            for chunk_entity in chunks.keys() {
                par_local.borrow_local_mut().push(*chunk_entity);
            }
        }
    });
    for entity in par_local.drain() {
        deduplicate_entities_hash_set.insert(entity);
    }

    // Gather all entities that need their specializations regenerated.
    // TODO: we get also get not visible chunks loaded here, do something to ignore them ?
    chunks_needing_specialization.par_iter().for_each(|entity| {
        par_local.borrow_local_mut().push(entity);
    });
    for entity in par_local.drain() {
        deduplicate_entities_hash_set.insert(entity);
    }

    entities_needing_specialization
        .changed
        .reserve(deduplicate_entities_hash_set.len());
    for entity in deduplicate_entities_hash_set.drain() {
        entities_needing_specialization
            .changed
            .push((entity, RenderEntity::from(Entity::PLACEHOLDER)));
    }

    for entity in deduplicate_entities_hash_set.drain() {
        entities_needing_specialization
            .removed
            .insert(entity, RenderEntity::from(Entity::PLACEHOLDER));
    }
}

pub struct SpecializationWorkItem {
    pub render_entity: Entity,
    pub visible_entity: MainEntity,
    pub retained_view_entity: RetainedViewEntity,
    pub mesh_key: MeshPipelineKey,
    pub splat_key: SplatPipelineKey,
    pub splat_layout: MeshVertexBufferLayoutRef,
    pub instance_layout: MeshVertexBufferLayoutRef,
    pub properties: Arc<PointCloudMaterialProperties>,
    pub view_settings_key: ErasedViewSettingsKey,
    pub material_type_id: TypeId,
}

#[derive(SystemParam)]
pub(crate) struct SpecializePointCloudMaterialsSystemParam<'w, 's> {
    render_meshes: Res<'w, RenderAssets<RenderMesh>>,
    render_materials: Res<'w, ErasedRenderAssets<PreparedPointCloudMaterial>>,
    render_mesh_instances: Res<'w, RenderMeshInstances>,
    render_material_instances: Res<'w, RenderPointCloudMaterialInstances>,
    render_point_cloud_instances: Res<'w, RenderPointCloudInstances>,
    render_point_cloud_chunk_instances: Res<'w, RenderPointCloudChunkInstances>,
    // render_lightmaps: Res<'w, RenderLightmaps>,
    render_visibility_ranges: Res<'w, RenderVisibilityRanges>,
    opaque_render_phases: Res<'w, ViewBinnedRenderPhases<Opaque3d>>,
    alpha_mask_render_phases: Res<'w, ViewBinnedRenderPhases<AlphaMask3d>>,
    transmissive_render_phases: Res<'w, ViewSortedRenderPhases<Transmissive3d>>,
    transparent_render_phases: Res<'w, ViewSortedRenderPhases<Transparent3d>>,
    views: Query<
        'w,
        's,
        (
            &'static ExtractedView,
            &'static RenderVisibleEntities,
            &'static ErasedMaterialViewSettingsKeys,
        ),
    >,
    view_key_cache: Res<'w, ViewKeyCache>,
    specialized_material_pipeline_cache: ResMut<'w, SpecializedPointCloudMaterialPipelineCache>,
    pending_mesh_material_queues: ResMut<'w, PendingPointCloudMaterialQueues>,
    dirty_specializations: Res<'w, PointCloudDirtySpecializations>,
}

pub(crate) fn specialize_point_cloud_materials(
    world: &mut World,
    state: &mut SystemState<SpecializePointCloudMaterialsSystemParam>,
    mut work_items: Local<Vec<SpecializationWorkItem>>,
    mut all_views: Local<HashSet<RetainedViewEntity, FixedHasher>>,
) {
    work_items.clear();
    all_views.clear();

    {
        let SpecializePointCloudMaterialsSystemParam {
            render_meshes,
            render_materials,
            render_mesh_instances,
            render_material_instances,
            render_point_cloud_instances,
            render_point_cloud_chunk_instances,
            // render_lightmaps,
            render_visibility_ranges,
            opaque_render_phases,
            alpha_mask_render_phases,
            transmissive_render_phases,
            transparent_render_phases,
            views,
            view_key_cache,
            mut specialized_material_pipeline_cache,
            mut pending_mesh_material_queues,
            dirty_specializations,
        } = state.get_mut(world).unwrap();

        for (view, visible_entities, view_settings_keys) in &views {
            all_views.insert(view.retained_view_entity);

            if !transparent_render_phases.contains_key(&view.retained_view_entity)
                && !opaque_render_phases.contains_key(&view.retained_view_entity)
                && !alpha_mask_render_phases.contains_key(&view.retained_view_entity)
                && !transmissive_render_phases.contains_key(&view.retained_view_entity)
            {
                continue;
            }

            let Some(view_key) = view_key_cache.get(&view.retained_view_entity) else {
                continue;
            };

            let Some(visible_entities_class) = visible_entities.get::<PointCloudChunk3d>() else {
                continue;
            };

            let mut maybe_specialized_material_pipeline_cache =
                specialized_material_pipeline_cache.get_mut(&view.retained_view_entity);

            // Remove cached pipeline IDs corresponding to entities that either
            // have been removed or need to be re-specialized.
            if let Some(ref mut specialized_material_pipeline_cache) =
                maybe_specialized_material_pipeline_cache
            {
                if dirty_specializations
                    .must_wipe_specializations_for_view(view.retained_view_entity)
                {
                    specialized_material_pipeline_cache.clear();
                } else {
                    for (&renderable_entity, &_main_entity) in
                        dirty_specializations.iter_to_despecialize()
                    {
                        specialized_material_pipeline_cache.remove(&renderable_entity);
                    }
                }
            }

            // Initialize the pending queues.
            let view_pending_mesh_material_queues =
                pending_mesh_material_queues.prepare_for_new_frame(view.retained_view_entity);

            // Now process all meshes that need to be specialized.
            for (render_entity, visible_entity) in dirty_specializations.iter_to_specialize(
                view.retained_view_entity,
                visible_entities_class,
                &view_pending_mesh_material_queues.prev_frame,
            ) {
                if maybe_specialized_material_pipeline_cache
                    .as_ref()
                    .is_some_and(|specialized_material_pipeline_cache| {
                        specialized_material_pipeline_cache.contains_key(render_entity)
                    })
                {
                    // the specialized material is already in cache, safe to continue
                    continue;
                }

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

                let Some(render_point_cloud_instance) = render_point_cloud_instances
                    .get(&render_point_cloud_chunk_instance.root_entity)
                else {
                    warn!(
                        "RenderPointCloudInstance not found for entity {:?}",
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
                        "Unable to load material instance for entity {:?}",
                        visible_entity
                    );
                    view_pending_mesh_material_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };

                // get the mesh instance from the root entity
                let Some(mesh_instance) = render_mesh_instances
                    .render_mesh_queue_data(render_point_cloud_chunk_instance.root_entity)
                else {
                    warn!(
                        "mesh_instance not found for entity {:?}",
                        render_point_cloud_chunk_instance.root_entity
                    );
                    view_pending_mesh_material_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };

                // get the mesh from the chunk
                let Some(mesh) = render_meshes.get(render_point_cloud_chunk_instance.mesh_asset_id)
                else {
                    warn!(
                        "render_meshes not found for asset id {:?}",
                        render_point_cloud_chunk_instance.mesh_asset_id
                    );
                    view_pending_mesh_material_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };

                let Some(material) = render_materials.get(material_instance.asset_id) else {
                    warn!("render_materials not found");
                    view_pending_mesh_material_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };

                let Some(splat_mesh) = render_meshes.get(render_point_cloud_instance.splat) else {
                    warn!("shape mesh not found");
                    view_pending_mesh_material_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };

                let mut mesh_pipeline_key_bits: MeshPipelineKey =
                    material.properties.mesh_pipeline_key_bits.downcast();
                mesh_pipeline_key_bits.insert(alpha_mode_pipeline_key(
                    material.properties.alpha_mode,
                    &Msaa::from_samples(view_key.msaa_samples()),
                ));
                let mut mesh_key = *view_key
                    | MeshPipelineKey::from_bits_retain(mesh.key_bits.bits())
                    | mesh_pipeline_key_bits;

                // if let Some(lightmap) = render_lightmaps.render_lightmaps.get(visible_entity) {
                //     mesh_key |= MeshPipelineKey::LIGHTMAPPED;

                //     if lightmap.bicubic_sampling {
                //         mesh_key |= MeshPipelineKey::LIGHTMAP_BICUBIC_SAMPLING;
                //     }
                // }

                if render_visibility_ranges
                    .entity_has_crossfading_visibility_ranges(*visible_entity)
                {
                    mesh_key |= MeshPipelineKey::VISIBILITY_RANGE_DITHER;
                }

                if view_key.contains(MeshPipelineKey::MOTION_VECTOR_PREPASS) {
                    if mesh_instance
                        .flags()
                        .contains(RenderMeshInstanceFlags::HAS_PREVIOUS_SKIN)
                    {
                        mesh_key |= MeshPipelineKey::HAS_PREVIOUS_SKIN;
                    }
                    if mesh_instance
                        .flags()
                        .contains(RenderMeshInstanceFlags::HAS_PREVIOUS_MORPH)
                    {
                        mesh_key |= MeshPipelineKey::HAS_PREVIOUS_MORPH;
                    }
                }

                let mut splat_key = (&render_point_cloud_instance.splat_settings).into();

                if matches!(
                    render_point_cloud_instance.topology,
                    PointCloudTopologyKind::Octree
                ) {
                    splat_key |= SplatPipelineKey::IS_OCTREE;
                }

                let Some(view_settings_key) = view_settings_keys
                    .view_settings_keys
                    .get(&material_instance.asset_id.type_id())
                else {
                    view_pending_mesh_material_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };

                work_items.push(SpecializationWorkItem {
                    render_entity: *render_entity,
                    visible_entity: *visible_entity,
                    retained_view_entity: view.retained_view_entity,
                    mesh_key,
                    splat_key,
                    splat_layout: splat_mesh.layout.clone(),
                    instance_layout: mesh.layout.clone(),
                    properties: material.properties.clone(),
                    view_settings_key: view_settings_key.clone(),
                    material_type_id: material_instance.asset_id.type_id(),
                });
            }
        }

        pending_mesh_material_queues.expire_stale_views(&all_views);
    }

    for item in work_items.drain(..) {
        let key = ErasedPointCloudMaterialPipelineKey {
            type_id: item.material_type_id,
            mesh_key: ErasedMeshPipelineKey::new(item.mesh_key),
            splat_key: ErasedSplatPipelineKey::new(item.splat_key),
            material_key: item.properties.material_key.clone(),
            view_settings_key: item.view_settings_key,
            pass: None,
        };

        let Some(base_specialize) = item.properties.base_specialize else {
            warn!("no base specialize");
            continue;
        };
        match base_specialize(
            world,
            key,
            &item.splat_layout,
            &item.instance_layout,
            &item.properties,
        ) {
            Ok(pipeline_id) => {
                world
                    .resource_mut::<SpecializedPointCloudMaterialPipelineCache>()
                    .entry(item.retained_view_entity)
                    .or_default()
                    .insert(item.render_entity, pipeline_id);
            }
            Err(err) => error!("{}", err),
        }
    }

    world
        .resource_mut::<SpecializedPointCloudMaterialPipelineCache>()
        .retain(|view, _| all_views.contains(view));
}

pub fn base_specialize(
    world: &mut World,
    key: ErasedPointCloudMaterialPipelineKey,
    splat_layout: &MeshVertexBufferLayoutRef,
    instance_layout: &MeshVertexBufferLayoutRef,
    properties: &Arc<PointCloudMaterialProperties>,
) -> Result<CachedRenderPipelineId, SpecializedMeshPipelineError> {
    world.resource_scope(
        |world,
         mut pipelines: Mut<
            SpecializedPointCloudPipelines<PointCloudMaterialPipelineSpecializer>,
        >| {
            let mesh_pipeline = world.resource::<PointCloudPipeline>().clone();
            let pipeline_cache = world.resource::<PipelineCache>();

            let specializer = PointCloudMaterialPipelineSpecializer {
                pipeline: PointCloudMaterialPipeline {
                    pointcloud_pipeline: mesh_pipeline,
                },
                properties: properties.clone(),
            };

            pipelines.specialize(
                pipeline_cache,
                &specializer,
                key,
                splat_layout,
                instance_layout,
            )
        },
    )
}

pub fn prepass_specialize(
    world: &mut World,
    key: ErasedPointCloudMaterialPipelineKey,
    splat_layout: &MeshVertexBufferLayoutRef,
    instance_layout: &MeshVertexBufferLayoutRef,
    properties: &Arc<PointCloudMaterialProperties>,
) -> Result<CachedRenderPipelineId, SpecializedMeshPipelineError> {
    world.resource_scope(
        |world, mut pipelines: Mut<SpecializedPointCloudPipelines<PrepassPipelineSpecializer>>| {
            let prepass_pipeline = world.resource::<PrepassPipeline>().clone();
            let pipeline_cache = world.resource::<PipelineCache>();

            let specializer = PrepassPipelineSpecializer {
                pipeline: prepass_pipeline,
                properties: properties.clone(),
            };

            pipelines.specialize(
                pipeline_cache,
                &specializer,
                key,
                splat_layout,
                instance_layout,
            )
        },
    )
}

pub fn user_specialize<M: PointCloudMaterial>(
    pipeline: &dyn Any,
    descriptor: &mut RenderPipelineDescriptor,
    splat_layout: &MeshVertexBufferLayoutRef,
    instance_layout: &MeshVertexBufferLayoutRef,
    erased_key: ErasedPointCloudMaterialPipelineKey,
) -> Result<(), SpecializedMeshPipelineError>
where
    M::Data: Hash + Clone,
{
    let pipeline = pipeline
        .downcast_ref::<PointCloudMaterialPipeline>()
        .unwrap();
    let material_key = erased_key.material_key.to_key();
    let mesh_key: MeshPipelineKey = erased_key.mesh_key.downcast();
    let splat_key: SplatPipelineKey = erased_key.splat_key.downcast();
    let view_settings_key = erased_key.view_settings_key.to_key();

    M::specialize(
        pipeline,
        descriptor,
        splat_layout,
        instance_layout,
        PointCloudMaterialPipelineKey {
            mesh_key,
            splat_key,
            bind_group_data: material_key,
            pass: erased_key.pass,
            view_settings_key,
        },
    )
}
