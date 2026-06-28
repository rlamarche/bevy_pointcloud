use std::any::TypeId;

use bevy::{
    camera::Camera,
    ecs::{
        entity::Entity,
        query::With,
        system::{Local, Query, ResMut},
    },
    log::warn,
    platform::collections::HashMap,
    render::{
        sync_world::{MainEntity, RenderEntity},
        view::{ExtractedView, RenderVisibleEntities},
        Extract,
    },
};

use crate::{
    ChildrenMask, NodeId, PointCloudChunk3d, RenderPointCloudInstanceIndex,
    RenderVisiblePointCloudChunkEntity, RenderVisiblePointCloudEntities, VisiblePointCloudEntities,
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
                .get_mut((render_entity.id(), MainEntity::from(main_entity)));

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

pub fn collect_visible_cpu_culled_point_cloud_chunk_entities(
    mut extracted_views: Query<
        (&mut RenderVisibleEntities, &RenderVisiblePointCloudEntities),
        With<ExtractedView>,
    >,
    // to preserve allocations
    mut visible_entities: Local<Vec<(Entity, MainEntity)>>,
) {
    for (mut render_visible_entities, render_visible_point_cloud_entities) in
        extracted_views.iter_mut()
    {
        // clear the visible entities for each view
        visible_entities.clear();

        for (_, render_visible_point_cloud_entity) in &render_visible_point_cloud_entities.entities
        {
            for render_visible_point_cloud_chunk_entity in
                &render_visible_point_cloud_entity.chunk_entities
            {
                visible_entities.push((
                    render_visible_point_cloud_chunk_entity.entity,
                    render_visible_point_cloud_chunk_entity.main_entity,
                ));
            }
        }

        // now update [`RenderVisibleEntities`] component of the extracted view

        // prepare render visible entities
        let entities = render_visible_entities
            .classes
            .entry(TypeId::of::<PointCloudChunk3d>())
            .or_default();

        entities.prepare_for_new_frame();

        // Make sure the entity list is sorted, as this is a requirement for
        // [`RenderVisibleEntitiesClass::update_from_cpu`].
        visible_entities.sort_unstable_by_key(|(_, main_entity)| *main_entity);

        entities.update_cpu_culled_entities(&visible_entities);
    }
}
