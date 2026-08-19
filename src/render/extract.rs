use std::any::TypeId;

use bevy::{
    asset::Assets,
    camera::{
        primitives::{Aabb, CascadesFrusta},
        visibility::{CascadesVisibleEntities, RenderLayers, ViewVisibility},
        Camera,
    },
    ecs::{
        entity::{ContainsEntity, Entity},
        hierarchy::ChildOf,
        query::{Changed, Has, Or, With, Without},
        system::{Local, Query, Res, ResMut},
    },
    light::{CascadeShadowConfig, Cascades, DirectionalLight, SpotLight, SunDisk, VolumetricLight},
    log::warn,
    pbr::{ExtractedDirectionalLight, PreviousGlobalTransform},
    platform::collections::HashMap,
    render::{
        mesh::{allocator::MeshAllocator, RenderMesh},
        occlusion_culling::OcclusionCulling,
        render_asset::RenderAssets,
        sync_world::{MainEntity, RenderEntity},
        view::{
            ExtractedView, RenderExtractedShadowMapVisibleEntities, RenderShadowMapVisibleEntities,
            RetainedViewEntity, VisibilityExtractionSystemParam,
        },
        Extract,
    },
    transform::components::GlobalTransform,
    utils::Parallel,
};

use crate::{
    CascadesVisiblePointCloudEntities, ChildrenMask, NodeId, PointCloud, PointCloud3d,
    PointCloudChunk, PointCloudChunk3d, PointCloudTopologyKind, PointCloudTransforms,
    RenderOctreeInstancesIndex, RenderPointCloudChunk, RenderPointCloudChunkInstance,
    RenderPointCloudChunkInstances, RenderPointCloudInstance, RenderPointCloudInstances,
    RenderShadowMapVisiblePointCloudEntities, RenderVisiblePointCloudChunkEntity,
    RenderVisiblePointCloudEntities, SplatMeshes, SplatSettings, VisiblePointCloudOctreeEntities,
};

/// This system extracts the visible point cloud chunk entities into the render world while
/// preserving the hierarchy, it also computes `first_child_index` and `children_mask` specific for
/// each views, for later visible nodes texture generation.
pub fn extract_visible_point_cloud_chunks(
    views: Extract<Query<(RenderEntity, &VisiblePointCloudOctreeEntities), With<Camera>>>,
    mut extracted_views: Query<&mut RenderVisiblePointCloudEntities, With<ExtractedView>>,
    mapper: Extract<Query<&RenderEntity>>,
    render_point_cloud_instances: Res<RenderPointCloudInstances>,
    render_point_cloud_chunks: Res<RenderAssets<RenderPointCloudChunk>>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    mesh_allocator: Res<MeshAllocator>,
    mut render_octree_index: ResMut<RenderOctreeInstancesIndex>,
) {
    for (render_entity, visible_point_cloud_entities) in views.iter() {
        let Ok(mut render_visible_point_cloud_entities) = extracted_views.get_mut(render_entity)
        else {
            warn!("Missing RenderVisiblePointCloudEntities for extracted view");
            continue;
        };

        // skip extraction if it has not changed
        if !visible_point_cloud_entities.changed_this_frame {
            render_visible_point_cloud_entities.changed_this_frame = false;
            continue;
        }

        // reset
        render_visible_point_cloud_entities.clear_all();

        // this index will contain, for each added chunk's node id, its index in the render visible
        // chunks
        let mut render_node_index = HashMap::<NodeId, usize>::new();

        for (&main_entity, point_cloud_entity) in &visible_point_cloud_entities.entities {
            let Ok(&render_entity) = mapper.get(main_entity) else {
                warn!("Render entity for PointCloud3d {} not found", main_entity);
                continue;
            };
            let main_entity = MainEntity::from(main_entity);

            let Some(render_point_cloud_instance) = render_point_cloud_instances.get(&main_entity)
            else {
                continue;
            };

            if matches!(
                render_point_cloud_instance.topology,
                PointCloudTopologyKind::Octree
            ) {
                // makes sure an index exists for this entity (used in visible texture nodes)
                render_octree_index.add(render_entity.id());
            }

            // get the render entry
            let render_visible_point_cloud_chunk_entity = render_visible_point_cloud_entities
                .get_or_insert_mut(main_entity, render_entity, point_cloud_entity.asset_id);

            // reset parent/child index
            render_node_index.clear();

            let mut sorted_octree_nodes = point_cloud_entity.node_entities.clone();

            // remove the missing nodes or unallocated nodes
            sorted_octree_nodes.retain(|node| {
                let Some(chunk_id) = node.chunk_id else {
                    return false;
                };
                let Some(render_chunk) = render_point_cloud_chunks.get(chunk_id) else {
                    return false;
                };

                let Some(mesh_handle) = render_chunk.mesh.as_ref() else {
                    return false;
                };

                if render_meshes.get(mesh_handle.id()).is_none() {
                    return false;
                };

                // TODO: is it needed to check this far?
                if mesh_allocator
                    .mesh_vertex_slice(&mesh_handle.id())
                    .is_none()
                {
                    return false;
                };

                true
            });

            sorted_octree_nodes
                .sort_unstable_by(|a, b| a.depth.cmp(&b.depth).then_with(|| a.name.cmp(&b.name)));

            for node_entity in &sorted_octree_nodes {
                if let (Some(chunk_entity), Some(chunk_id)) =
                    (node_entity.entity, node_entity.chunk_id)
                {
                    let current_index =
                        render_visible_point_cloud_chunk_entity.chunk_entities.len();

                    // we store the future index of the node
                    render_node_index.insert(node_entity.id, current_index);

                    // if there is a parent, update its children_mask / children array
                    if let Some(parent_node_id) = node_entity.parent_id
                        && let Some(&parent_index) = render_node_index.get(&parent_node_id)
                    {
                        let parent_chunk = &mut render_visible_point_cloud_chunk_entity
                            .chunk_entities[parent_index];

                        parent_chunk.children_mask |= node_entity.child_index.into();
                        parent_chunk.children[node_entity
                            .child_index
                            .index()
                            .expect("Trying to convert child index which isn't a valid index.")
                            as usize] = current_index;
                        if current_index < parent_chunk.first_child_index {
                            parent_chunk.first_child_index = current_index;
                        }
                    }

                    let render_entity = mapper
                        .get(chunk_entity)
                        .expect("render entity not available for a chunk entity")
                        .id();

                    let main_entity = MainEntity::from(chunk_entity);

                    // transform offset in u8
                    let offset =
                        ((node_entity.offset.unwrap_or(0.0) + 10.0) * 10.0).min(255.0) as u8;

                    // insert the visible chunk in
                    render_visible_point_cloud_chunk_entity.chunk_entities.push(
                        RenderVisiblePointCloudChunkEntity {
                            id: node_entity.id,
                            chunk_id,
                            name: node_entity.name.clone(),
                            parent_id: node_entity.parent_id,
                            depth: node_entity.depth,
                            offset,
                            child_index: node_entity.child_index,
                            first_child_index: usize::MAX,
                            // empty children list, will be filled when adding children
                            children: [0; 8],
                            // same here, will be recomputed
                            children_mask: ChildrenMask::empty(),
                            entity: render_entity,
                            main_entity,
                        },
                    );
                }
            }
        }
    }
}

/// This system extracts the visible point cloud chunk entities for each light, view & cascade, into
/// the render world while preserving the hierarchy, it also computes `first_child_index` and
/// `children_mask` specific for each views, for later visible nodes texture generation.
pub fn extract_cascade_visible_point_cloud_chunks(
    views: Extract<
        Query<(Entity, RenderEntity, &CascadesVisiblePointCloudEntities), With<DirectionalLight>>,
    >,
    // mut extracted_views: Query<&mut RenderShadowMapVisiblePointCloudEntities,
    // With<ExtractedView>>,
    mut extracted_directional_lights: Query<(
        Entity,
        &MainEntity,
        &ExtractedDirectionalLight,
        &RenderLayers,
        &mut RenderShadowMapVisiblePointCloudEntities,
    )>,
    mapper: Extract<Query<&RenderEntity>>,
    render_point_cloud_instances: Res<RenderPointCloudInstances>,
    render_point_cloud_chunks: Res<RenderAssets<RenderPointCloudChunk>>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    mesh_allocator: Res<MeshAllocator>,
    mut render_octree_index: ResMut<RenderOctreeInstancesIndex>,
) {
    for (main_entity, render_entity, cascade_visible_point_cloud_entities) in views.iter() {
        let Ok((_, _, _, _, mut render_shadow_map_visible_point_cloud_entities)) =
            extracted_directional_lights.get_mut(render_entity)
        else {
            warn!("Missing extracted directional light {:?}", render_entity);
            continue;
        };

        for (&main_auxiliary_entity, visible_point_cloud_entities) in
            &cascade_visible_point_cloud_entities.entities
        {
            for (subview_index, (_, visible_point_cloud_octree_entities)) in
                visible_point_cloud_entities.iter().enumerate()
            {
                let retained_view_entity = RetainedViewEntity {
                    main_entity: MainEntity::from(main_entity),
                    auxiliary_entity: MainEntity::from(main_auxiliary_entity),
                    subview_index: subview_index as u32,
                };

                let mut render_visible_point_cloud_entities =
                    render_shadow_map_visible_point_cloud_entities
                        .subviews
                        .entry(retained_view_entity)
                        .or_default();

                if !visible_point_cloud_octree_entities.changed_this_frame {
                    render_visible_point_cloud_entities.changed_this_frame = false;
                    continue;
                }

                // reset
                render_visible_point_cloud_entities.clear_all();

                // this index will contain, for each added chunk's node id, its index in the render
                // visible chunks
                let mut render_node_index = HashMap::<NodeId, usize>::new();

                for (&main_entity, point_cloud_entity) in
                    &visible_point_cloud_octree_entities.entities
                {
                    let Ok(&render_entity) = mapper.get(main_entity) else {
                        warn!("Render entity for PointCloud3d {} not found", main_entity);
                        continue;
                    };
                    let main_entity = MainEntity::from(main_entity);

                    let Some(render_point_cloud_instance) =
                        render_point_cloud_instances.get(&main_entity)
                    else {
                        continue;
                    };

                    if matches!(
                        render_point_cloud_instance.topology,
                        PointCloudTopologyKind::Octree
                    ) {
                        // makes sure an index exists for this entity (used in visible texture
                        // nodes)
                        render_octree_index.add(render_entity.id());
                    }

                    // get the render entry
                    let render_visible_point_cloud_chunk_entity =
                        render_visible_point_cloud_entities.get_or_insert_mut(
                            main_entity,
                            render_entity,
                            point_cloud_entity.asset_id,
                        );

                    // reset parent/child index
                    render_node_index.clear();

                    let mut sorted_octree_nodes = point_cloud_entity.node_entities.clone();

                    // remove the missing nodes or unallocated nodes
                    sorted_octree_nodes.retain(|node| {
                        let Some(chunk_id) = node.chunk_id else {
                            return false;
                        };
                        let Some(render_chunk) = render_point_cloud_chunks.get(chunk_id) else {
                            return false;
                        };

                        let Some(mesh_handle) = render_chunk.mesh.as_ref() else {
                            return false;
                        };

                        if render_meshes.get(mesh_handle.id()).is_none() {
                            return false;
                        };

                        // TODO: is it needed to check this far?
                        if mesh_allocator
                            .mesh_vertex_slice(&mesh_handle.id())
                            .is_none()
                        {
                            return false;
                        };

                        true
                    });

                    sorted_octree_nodes.sort_unstable_by(|a, b| {
                        a.depth.cmp(&b.depth).then_with(|| a.name.cmp(&b.name))
                    });

                    for node_entity in &sorted_octree_nodes {
                        if let (Some(chunk_entity), Some(chunk_id)) =
                            (node_entity.entity, node_entity.chunk_id)
                        {
                            let current_index =
                                render_visible_point_cloud_chunk_entity.chunk_entities.len();

                            // we store the future index of the node
                            render_node_index.insert(node_entity.id, current_index);

                            // if there is a parent, update its children_mask / children array
                            if let Some(parent_node_id) = node_entity.parent_id
                                && let Some(&parent_index) = render_node_index.get(&parent_node_id)
                            {
                                let parent_chunk = &mut render_visible_point_cloud_chunk_entity
                                    .chunk_entities[parent_index];

                                parent_chunk.children_mask |= node_entity.child_index.into();
                                parent_chunk.children[node_entity.child_index.index().expect(
                                    "Trying to convert child index which isn't a valid index.",
                                ) as usize] = current_index;
                                if current_index < parent_chunk.first_child_index {
                                    parent_chunk.first_child_index = current_index;
                                }
                            }

                            let render_entity = mapper
                                .get(chunk_entity)
                                .expect("render entity not available for a chunk entity")
                                .id();

                            let main_entity = MainEntity::from(chunk_entity);

                            // transform offset in u8
                            let offset = ((node_entity.offset.unwrap_or(0.0) + 10.0) * 10.0)
                                .min(255.0) as u8;

                            // insert the visible chunk in
                            render_visible_point_cloud_chunk_entity.chunk_entities.push(
                                RenderVisiblePointCloudChunkEntity {
                                    id: node_entity.id,
                                    chunk_id,
                                    name: node_entity.name.clone(),
                                    parent_id: node_entity.parent_id,
                                    depth: node_entity.depth,
                                    offset,
                                    child_index: node_entity.child_index,
                                    first_child_index: usize::MAX,
                                    // empty children list, will be filled when adding children
                                    children: [0; 8],
                                    // same here, will be recomputed
                                    children_mask: ChildrenMask::empty(),
                                    entity: render_entity,
                                    main_entity,
                                },
                            );
                        }
                    }
                }
            }
        }
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
    entities: Extract<
        Query<(
            Entity,
            &PointCloud3d,
            &SplatSettings,
            Option<&Aabb>,
            &ViewVisibility,
            &GlobalTransform,
            Option<&PreviousGlobalTransform>,
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
            render_layers,
        )| {
            if !view_visibility.get() {
                return;
            }

            let Some(aabb) = maybe_aabb else {
                warn!(
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
                warn!("Render entity for PointCloud3d {} not found", entity);
                return;
            };

            queue.push((
                entity,
                RenderPointCloudInstance {
                    entity: entity.into(),
                    render_entity: render_entity.id(),
                    aabb: *aabb,
                    model_aabb: point_cloud.aabb.unwrap_or_default(),
                    spacing: point_cloud.spacing,
                    topology: (&point_cloud.topology).into(),
                    transforms: PointCloudTransforms {
                        world_from_local: world_from_local.into(),
                        previous_world_from_local: previous_world_from_local.into(),
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
        Query<(
            Entity,
            &PointCloudChunk3d,
            Option<&ChildOf>,
            &ViewVisibility,
            // to determine if it is a root
            Has<PointCloud3d>,
        )>,
    >,
    chunks: Extract<Res<Assets<PointCloudChunk>>>,
) {
    // TODO: most fields are invariant, consider extract only new items.
    // TODO: and remove non visible instances on the next iteration (because we need the instance to
    // remove it from the phase when looking up for the root entity).
    chunks_query.par_iter().for_each_init(
        || render_point_cloud_chunk_instance_queues.borrow_local_mut(),
        |queue, (entity, point_cloud_chunk_3d, maybe_child_of, _view_visibility, is_root)| {
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
                    topology: chunk.topology,
                },
            ));
        },
    );

    // Collect the render mesh instances.
    render_point_cloud_chunk_instances.clear();
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
                        auxiliary_entity: MainEntity::from(*main_auxiliary_entity),
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
