use std::any::TypeId;

use bevy::{
    asset::Assets,
    camera::{
        primitives::{Aabb, CascadesFrusta},
        visibility::{
            CascadesVisibleEntities, NoFrustumCulling, RenderLayers, ViewVisibility,
            VisibilityRange,
        },
    },
    ecs::{
        entity::{ContainsEntity, Entity},
        hierarchy::ChildOf,
        lifecycle::RemovedComponents,
        query::{Changed, Has, Or, With, Without},
        system::{Local, Query, Res, ResMut},
    },
    light::{
        CascadeShadowConfig, Cascades, DirectionalLight, NotShadowReceiver, SpotLight, SunDisk,
        TransmittedShadowReceiver, VolumetricLight,
    },
    log::{debug, warn},
    pbr::{MeshFlags, MeshTransforms, PreviousGlobalTransform},
    render::{
        occlusion_culling::OcclusionCulling,
        sync_world::{MainEntity, RenderEntity},
        view::{
            RenderExtractedShadowMapVisibleEntities, RenderShadowMapVisibleEntities,
            RenderVisibilityRanges, RetainedViewEntity, VisibilityExtractionSystemParam,
        },
        Extract,
    },
    transform::components::GlobalTransform,
    utils::Parallel,
};
use nonmax::NonMaxU16;

use crate::{
    render::{
        PreparedPointCloudUniforms, RenderOctreeInstancesIndex, RenderPointCloudChunkInstance,
        RenderPointCloudChunkInstances, RenderPointCloudInstance, RenderPointCloudInstances,
        SplatMeshes,
    },
    CascadesVisiblePointCloudEntities, PointCloud, PointCloud3d, PointCloudChunk,
    PointCloudChunk3d, SplatSettings,
};

pub fn update_render_octree_index(
    visible_point_clouds: Extract<Query<Entity, (With<PointCloud3d>, With<ViewVisibility>)>>,
    mut removed_main_entities: Extract<RemovedComponents<PointCloud3d>>,
    mut render_octree_index: ResMut<RenderOctreeInstancesIndex>,
    mut prepared_point_cloud_uniforms: ResMut<PreparedPointCloudUniforms>,
) {
    let removed_main_entities = removed_main_entities
        .read()
        .map(MainEntity::from)
        .collect::<Vec<_>>();
    for removed_main_entity in &removed_main_entities {
        prepared_point_cloud_uniforms.remove(removed_main_entity);
        render_octree_index.remove(&removed_main_entity.id());
    }

    for visible_point_cloud in visible_point_clouds.into_iter() {
        render_octree_index.add(visible_point_cloud);
    }
}

/// Extracts meshes from the main world into the render world, populating the
/// [`RenderPointCloudInstances`] resource, which contains a [`RenderPointCloudInstance`]
/// for each visible [`PointCloud3d`].
///
/// It also extracts the its aabb (useful for rendering features), and its transforms, for
/// populating the [`crate::PointCloudUniform`] later.
/// TODO: extract only if changed
pub fn extract_pointcloud_instances(
    mut render_point_cloud_instances: ResMut<RenderPointCloudInstances>,
    mut render_point_cloud_instance_queues: Local<
        Parallel<Vec<(Entity, RenderPointCloudInstance)>>,
    >,
    render_visibility_ranges: Res<RenderVisibilityRanges>,
    entities: Extract<
        Query<(
            Entity,
            &PointCloud3d,
            &SplatSettings,
            Option<&Aabb>,
            &ViewVisibility,
            &GlobalTransform,
            Option<&PreviousGlobalTransform>,
            Has<NoFrustumCulling>,
            Has<NotShadowReceiver>,
            Has<TransmittedShadowReceiver>,
            Option<&VisibilityRange>,
            Option<&RenderLayers>,
        )>,
    >,
    mapper: Extract<Query<&RenderEntity>>,
    point_clouds: Extract<Res<Assets<PointCloud>>>,
    splat_meshes: Extract<Res<SplatMeshes>>,
) {
    entities.par_iter().for_each_init(
        || render_point_cloud_instance_queues.borrow_local_mut(),
        |queue,
         (
            entity,
            point_cloud_3d,
            point_cloud_splat_settings,
            maybe_aabb,
            view_visibility,
            transform,
            previous_transform,
            no_frustum_culling,
            not_shadow_receiver,
            transmitted_receiver,
            visibility_range,
            render_layers,
        )| {
            if !view_visibility.get() {
                return;
            }

            let Some(aabb) = maybe_aabb else {
                debug!(
                    "Point cloud's aabb of render entity {:?} not yet available.",
                    entity
                );
                return;
            };

            let world_from_local = transform.affine();
            let previous_world_from_local = previous_transform
                .map(|previous_transform| previous_transform.0)
                .unwrap_or(world_from_local);

            let Some(point_cloud) = point_clouds.get(point_cloud_3d) else {
                warn!("Point Cloud {:?} not found", point_cloud_3d.id());
                return;
            };

            let Ok(render_entity) = mapper.get(entity) else {
                warn!("Render entity for PointCloud3d {} not found 2", entity);
                return;
            };

            let mut lod_index = None;
            if visibility_range.is_some() {
                lod_index = render_visibility_ranges.lod_index_for_entity(entity.into());
            }

            let mesh_flags = mesh_flags_from_components(
                transform,
                lod_index,
                visibility_range,
                no_frustum_culling,
                not_shadow_receiver,
                transmitted_receiver,
            );

            queue.push((
                entity,
                RenderPointCloudInstance {
                    entity: entity.into(),
                    render_entity: render_entity.id(),
                    aabb: *aabb,
                    model_aabb: point_cloud.aabb.unwrap_or_default(),
                    spacing: point_cloud.spacing,
                    topology: (&point_cloud.topology).into(),
                    transforms: MeshTransforms {
                        world_from_local: world_from_local.into(),
                        previous_world_from_local: previous_world_from_local.into(),
                        flags: mesh_flags.bits(),
                    },
                    render_layers: render_layers.cloned(),
                    // TODO put default asset id if not filled
                    splat_settings: point_cloud_splat_settings.clone(),
                    splat: match &point_cloud_splat_settings.splat {
                        Some(handle) => handle.id(),
                        None => splat_meshes.quad_mesh.id(),
                    },
                },
            ));
        },
    );

    // Collect the render mesh instances.
    render_point_cloud_instances.clear();
    for queue in render_point_cloud_instance_queues.iter_mut() {
        for (entity, render_mesh_instance) in queue.drain(..) {
            render_point_cloud_instances.insert(entity.into(), render_mesh_instance);
        }
    }
}

/// Extracts meshes from the main world into the render world, populating the
/// [`RenderPointCloudChunkInstances`] resource, which contains a [`RenderPointCloudChunkInstance`]
/// for each visible [`PointCloudChunk3d`].
/// Note: for the moment, invisible [`PointCloudChunk3d`] are also extracted.
pub fn extract_pointcloud_chunk_instances(
    mut render_point_cloud_chunk_instances: ResMut<RenderPointCloudChunkInstances>,
    mut render_point_cloud_chunk_instance_queues: Local<
        Parallel<Vec<(Entity, RenderPointCloudChunkInstance)>>,
    >,
    mapper: Extract<Query<&RenderEntity>>,
    chunks_query: Extract<
        Query<
            (
                Entity,
                &PointCloudChunk3d,
                Option<&ChildOf>,
                // to determine if it is a root
                Has<PointCloud3d>,
            ),
            Changed<PointCloudChunk3d>,
        >,
    >,
    chunks: Extract<Res<Assets<PointCloudChunk>>>,
) {
    // TODO: most fields are invariant, consider extract only new items.
    // TODO: and remove non visible instances on the next iteration (because we need the instance to
    // remove it from the phase when looking up for the root entity).
    chunks_query.par_iter().for_each_init(
        || render_point_cloud_chunk_instance_queues.borrow_local_mut(),
        |queue, (entity, point_cloud_chunk_3d, maybe_child_of, is_root)| {
            // we keep invisible instances for the moment because we need the instance to remove
            // phases
            // if !view_visibility.get() {
            //     return;
            // }

            let root_entity = match is_root {
                true => entity,
                false => match maybe_child_of {
                    Some(ChildOf(parent_entity)) => *parent_entity,
                    None => {
                        warn!("Not root entity found for chunk {:?}", entity);
                        return;
                    }
                },
            };

            let Some(chunk) = chunks.get(point_cloud_chunk_3d) else {
                warn!("Chunk asset not found for chunk {:?}", point_cloud_chunk_3d);
                return;
            };
            let Some(mesh_handle) = &chunk.mesh_handle else {
                warn!("Mesh handle missing for chunk {:?}", point_cloud_chunk_3d);
                return;
            };

            queue.push((
                entity,
                RenderPointCloudChunkInstance {
                    is_root,
                    root_entity: root_entity.into(),
                    mesh_asset_id: mesh_handle.id(),
                    topology: chunk.topology.clone(),
                },
            ));
        },
    );

    // Collect the render mesh instances
    for queue in render_point_cloud_chunk_instance_queues.iter_mut() {
        for (entity, render_point_cloud_chunk_instance) in queue.drain(..) {
            let Ok(render_entity) = mapper.get(entity) else {
                warn!("Render entity not found for main entity {:?}", entity);
                continue;
            };
            render_point_cloud_chunk_instances
                .insert(render_entity.entity(), render_point_cloud_chunk_instance);
        }
    }
}

pub fn extract_lights_visible_point_cloud_chunks(
    directional_lights: Extract<
        Query<
            (
                Entity,
                RenderEntity,
                &DirectionalLight,
                &CascadesVisiblePointCloudEntities,
                &CascadeShadowConfig,
                &ViewVisibility,
            ),
            (
                Without<SpotLight>,
                Or<(
                    Changed<DirectionalLight>,
                    Changed<CascadesVisibleEntities>,
                    Changed<CascadesVisiblePointCloudEntities>,
                    Changed<Cascades>,
                    Changed<CascadeShadowConfig>,
                    Changed<CascadesFrusta>,
                    Changed<GlobalTransform>,
                    Changed<ViewVisibility>,
                    Changed<RenderLayers>,
                    Changed<VolumetricLight>,
                    Changed<OcclusionCulling>,
                    Changed<SunDisk>,
                )>,
            ),
        >,
    >,
    visibility_extraction_system_param: VisibilityExtractionSystemParam,
    mut existing_render_shadow_map_visible_entities: Query<(
        &mut RenderExtractedShadowMapVisibleEntities,
        &mut RenderShadowMapVisibleEntities,
    )>,
) {
    let mapper = &visibility_extraction_system_param.mapper;

    for (
        main_entity,
        entity,
        directional_light,
        visible_entities,
        cascade_config,
        view_visibility,
    ) in &directional_lights
    {
        if !view_visibility.get() {
            continue;
        }

        if directional_light.shadow_maps_enabled {
            let Ok((
                mut existing_extracted_shadow_map_visible_entities,
                mut existing_shadow_map_visible_entities,
            )) = existing_render_shadow_map_visible_entities.get_mut(entity)
            else {
                // It happens on the first loop only because the resource is not yet populated with
                // shadow maps.
                continue;
            };

            // Calculate the added and removed entities for each cascade.
            for (main_auxiliary_entity, visible_mesh_entities_list) in
                visible_entities.entities.iter()
            {
                for subview_index in 0..(cascade_config.bounds.len() as u32) {
                    let retained_view_entity = RetainedViewEntity {
                        main_entity: MainEntity::from(main_entity),
                        auxiliary_entity: (*main_auxiliary_entity != Entity::PLACEHOLDER)
                            .then_some(MainEntity::from(*main_auxiliary_entity)),
                        subview_index,
                    };

                    existing_shadow_map_visible_entities
                        .subviews
                        .entry(retained_view_entity)
                        .or_default();

                    // Extract the visible CPU culled entities to the list.
                    let extracted_entities = &mut existing_extracted_shadow_map_visible_entities
                        .subviews
                        .entry(retained_view_entity)
                        .or_default()
                        .classes
                        .entry(TypeId::of::<PointCloudChunk3d>())
                        .or_default()
                        .entities;
                    extracted_entities.clear();
                    let Some((visible_flat_chunk_entities, visible_octree_chunk_entities)) =
                        visible_mesh_entities_list.get(subview_index as usize)
                    else {
                        continue;
                    };
                    extracted_entities.extend(
                        visible_flat_chunk_entities.entities.iter().flat_map(
                            |(&main_entity, _)| {
                                let render_entity = match mapper.get(main_entity) {
                                    Ok(render_entity) => **render_entity,
                                    Err(_) => Entity::PLACEHOLDER,
                                };
                                Some((render_entity, MainEntity::from(main_entity)))
                            },
                        ),
                    );
                    extracted_entities.extend(
                        visible_octree_chunk_entities.entities.iter().flat_map(
                            |(_, visible_point_cloud_entity)| {
                                // get all node entities which have an associated entity
                                visible_point_cloud_entity
                                    .node_entities
                                    .iter()
                                    .flat_map(|node| {
                                        let main_entity = node.entity?;
                                        let render_entity = match mapper.get(main_entity) {
                                            Ok(render_entity) => **render_entity,
                                            Err(_) => Entity::PLACEHOLDER,
                                        };
                                        Some((render_entity, MainEntity::from(main_entity)))
                                    })
                            },
                        ),
                    );
                }
            }
        }
    }
}

/// Copied from [`MeshFlags::from_components`]
fn mesh_flags_from_components(
    transform: &GlobalTransform,
    lod_index: Option<NonMaxU16>,
    visibility_range: Option<&VisibilityRange>,
    no_frustum_culling: bool,
    not_shadow_receiver: bool,
    transmitted_receiver: bool,
) -> MeshFlags {
    let mut mesh_flags = if not_shadow_receiver {
        MeshFlags::empty()
    } else {
        MeshFlags::SHADOW_RECEIVER
    };
    if visibility_range.is_some_and(|visibility_range| visibility_range.use_aabb) {
        mesh_flags |= MeshFlags::AABB_BASED_VISIBILITY_RANGE;
    }
    if no_frustum_culling {
        mesh_flags |= MeshFlags::NO_FRUSTUM_CULLING;
    }
    if transmitted_receiver {
        mesh_flags |= MeshFlags::TRANSMITTED_SHADOW_RECEIVER;
    }
    if transform.affine().matrix3.determinant().is_sign_positive() {
        mesh_flags |= MeshFlags::SIGN_DETERMINANT_MODEL_3X3;
    }

    let lod_index_bits = match lod_index {
        None => u16::MAX,
        Some(lod_index) => u16::from(lod_index),
    };
    mesh_flags |=
        MeshFlags::from_bits_retain((lod_index_bits as u32) << MeshFlags::LOD_INDEX_SHIFT);

    mesh_flags
}

// /// Free allocated buffers & bindgroups for removed point clouds
// pub fn free_removed_point_cloud_uniforms(
//     mut removed_items: Extract<RemovedComponents<PointCloud3d>>,
//     mut prepared_point_cloud_uniforms: ResMut<PreparedPointCloudUniforms>,
//     mut view_point_clouds_bind_groups: Query<&mut ViewPointCloudBindGroups>,
// ) {
//     for entity in removed_items.read() {
//         let main_entity = MainEntity::from(entity);
//         for mut view_point_cloud_bind_groups in &mut view_point_clouds_bind_groups {
//             for (_, bind_groups) in view_point_cloud_bind_groups.bind_groups.iter_mut() {
//                 bind_groups.remove(&main_entity);
//             }
//         }
//         prepared_point_cloud_uniforms.remove(&main_entity);
//     }
// }
