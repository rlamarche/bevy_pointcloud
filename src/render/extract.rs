use bevy::{
    camera::{
        primitives::Aabb,
        visibility::{RenderLayers, ViewVisibility},
        Camera,
    },
    ecs::{
        entity::Entity,
        hierarchy::ChildOf,
        query::With,
        system::{Local, Query, ResMut},
    },
    log::warn,
    mesh::Mesh3d,
    pbr::PreviousGlobalTransform,
    platform::collections::HashMap,
    render::{
        sync_world::{MainEntity, RenderEntity},
        view::ExtractedView,
        Extract,
    },
    transform::components::GlobalTransform,
    utils::Parallel,
};

use crate::{
    ChildrenMask, NodeId, PointCloudChunk3d, PointCloudTransforms, RenderPointCloudInstance,
    RenderPointCloudInstanceIndex, RenderPointCloudInstances, RenderVisiblePointCloudChunkEntity,
    RenderVisiblePointCloudEntities, VisiblePointCloudEntities,
};

pub fn extract_visible_point_cloud_chunks(
    views: Extract<Query<(RenderEntity, &VisiblePointCloudEntities), With<Camera>>>,
    mut extracted_views: Query<&mut RenderVisiblePointCloudEntities, With<ExtractedView>>,
    mapper: Extract<Query<&RenderEntity>>,
    mut render_point_cloud_index: ResMut<RenderPointCloudInstanceIndex>,
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

            // makes sure an index exists for this entity (used in visible texture nodes)
            render_point_cloud_index.add(render_entity.id());

            // get the render entry
            let render_visible_point_cloud_chunk_entity = render_visible_point_cloud_entities
                .get_or_insert_mut(
                    MainEntity::from(main_entity),
                    render_entity,
                    point_cloud_entity.asset_id,
                );

            // reset parent/child index
            render_node_index.clear();

            for node_entity in &point_cloud_entity.node_entities {
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
                        parent_chunk.children[node_entity.child_index.index()] = current_index;
                    }

                    let render_entity = mapper
                        .get(chunk_entity)
                        .expect("render entity not available for a chunk entity")
                        .id();

                    let main_entity = MainEntity::from(chunk_entity);

                    // insert the visible chunk in
                    render_visible_point_cloud_chunk_entity.chunk_entities.push(
                        RenderVisiblePointCloudChunkEntity {
                            id: node_entity.id,
                            chunk_id,
                            name: node_entity.name.clone(),
                            parent_id: node_entity.parent_id,
                            depth: node_entity.depth,
                            child_index: node_entity.child_index,
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

/// Extracts meshes from the main world into the render world, populating the
/// [`RenderMeshInstances`].
///
/// This is the variant of the system that runs when we're *not* using GPU
/// [`MeshUniform`] building.
pub fn extract_pointcloud_chunks(
    mut render_mesh_instances: ResMut<RenderPointCloudInstances>,
    mut render_mesh_instance_queues: Local<Parallel<Vec<(Entity, RenderPointCloudInstance)>>>,
    meshes_query: Extract<
        Query<(
            Entity,
            Option<&ChildOf>,
            Option<&Aabb>,
            &ViewVisibility,
            &GlobalTransform,
            &Mesh3d,
            &PointCloudChunk3d,
            Option<&PreviousGlobalTransform>,
            Option<&RenderLayers>,
        )>,
    >,
    aabb_query: Extract<Query<&Aabb>>,
) {
    meshes_query.par_iter().for_each_init(
        || render_mesh_instance_queues.borrow_local_mut(),
        |queue,
         (
            entity,
            maybe_child_of,
            maybe_aabb,
            view_visibility,
            transform,
            mesh,
            chunk,
            previous_transform,
            render_layers,
        )| {
            if !view_visibility.get() {
                return;
            }

            let Some(aabb) = (match maybe_aabb {
                Some(aabb) => Some(aabb),
                None => match maybe_child_of {
                    Some(child_of) => aabb_query.get(child_of.parent()).ok(),
                    None => None,
                },
            }) else {
                warn!("Unable to get chunk's root aabb");
                return;
            };

            let world_from_local = transform.affine();
            let previous_world_from_local = previous_transform
                .map(|previous_transform| previous_transform.0)
                .unwrap_or(world_from_local);

            queue.push((
                entity,
                RenderPointCloudInstance {
                    aabb: *aabb,
                    mesh_id: mesh.id(),
                    asset_id: chunk.id(),
                    transforms: PointCloudTransforms {
                        world_from_local: world_from_local.into(),
                        previous_world_from_local: previous_world_from_local.into(),
                    },
                    render_layers: render_layers.cloned(),
                },
            ));
        },
    );

    // Collect the render mesh instances.
    render_mesh_instances.clear();
    for queue in render_mesh_instance_queues.iter_mut() {
        for (entity, render_mesh_instance) in queue.drain(..) {
            render_mesh_instances.insert(entity.into(), render_mesh_instance);
        }
    }
}

// pub fn collect_visible_cpu_culled_point_cloud_chunk_entities(
//     mut extracted_views: Query<
//         (&mut RenderVisibleEntities, &RenderVisiblePointCloudEntities),
//         With<ExtractedView>,
//     >,
//     // to preserve allocations
//     mut visible_entities: Local<Vec<(Entity, MainEntity)>>,
// ) {
//     for (mut render_visible_entities, render_visible_point_cloud_entities) in
//         extracted_views.iter_mut()
//     {
//         // clear the visible entities for each view
//         visible_entities.clear();

//         for (_, render_visible_point_cloud_entity) in
// &render_visible_point_cloud_entities.entities         {
//             for render_visible_point_cloud_chunk_entity in
//                 &render_visible_point_cloud_entity.chunk_entities
//             {
//                 visible_entities.push((
//                     render_visible_point_cloud_chunk_entity.entity,
//                     render_visible_point_cloud_chunk_entity.main_entity,
//                 ));
//             }
//         }

//         // now update [`RenderVisibleEntities`] component of the extracted view

//         // prepare render visible entities
//         let entities = render_visible_entities
//             .classes
//             .entry(TypeId::of::<PointCloudChunk3d>())
//             .or_default();

//         entities.prepare_for_new_frame();

//         // Make sure the entity list is sorted, as this is a requirement for
//         // [`RenderVisibleEntitiesClass::update_from_cpu`].
//         visible_entities.sort_unstable_by_key(|(_, main_entity)| *main_entity);

//         entities.update_cpu_culled_entities(&visible_entities);
//     }
// }
