use bevy::{ecs::{entity::Entity, resource::Resource, system::{Query, ResMut}}, log::info, platform::collections::HashSet, render::{sync_world::{MainEntity, MainEntityHashSet}, view::{ExtractedView, RenderVisibleEntitiesClass, RetainedViewEntity}}};
use itertools::Either;


/// Clears out the [`DirtySpecializations`] resource in preparation for a new
/// frame.
pub fn clear_dirty_specializations(mut dirty_specializations: ResMut<DirtySpecializations>) {
    dirty_specializations.changed_renderables.clear();
    dirty_specializations.removed_renderables.clear();
    dirty_specializations.views.clear();
}


/// A system that removes views that don't exist any longer from
/// [`DirtySpecializations`].
pub fn expire_specializations_for_views(
    views: Query<&ExtractedView>,
    mut dirty_specializations: ResMut<DirtySpecializations>,
) {
    let all_live_retained_view_entities: HashSet<_> =
        views.iter().map(|view| view.retained_view_entity).collect();
    dirty_specializations.views.retain(|retained_view_entity| {
        all_live_retained_view_entities.contains(retained_view_entity)
    });
}

/// Duplicated from Bevy to prevent collisions
/// Stores information about all entities that have changed in such a way as to
/// potentially require their pipelines to be re-specialized.
///
/// This is conservative; there's no harm, other than performance, in having an
/// entity in this list that doesn't actually need to be re-specialized. Note
/// that the presence of an entity in this list doesn't mean that a new shader
/// will necessarily be compiled; the pipeline cache is checked first.
///
/// This handles 2D meshes, 3D meshes, and sprites. For 2D and 3D wireframes,
/// see [`DirtyWireframeSpecializations`]. The reason for having two separate
/// lists is that a single entity can have both a mesh and a wireframe.
#[derive(Clone, Resource, Default)]
pub struct DirtySpecializations {
    /// All renderable objects that must be re-specialized this frame.
    pub changed_renderables: MainEntityHashSet,

    /// All renderable objects that need their specializations removed this
    /// frame.
    ///
    /// Note that this may include entities in [`Self::changed_renderables`].
    /// This is fine, as old specializations are removed before new ones are
    /// added.
    pub removed_renderables: MainEntityHashSet,

    /// Views that must be respecialized this frame.
    ///
    /// The presence of a view in this list causes all entities that it renders
    /// to be re-specialized.
    pub views: HashSet<RetainedViewEntity>,
}

impl DirtySpecializations {
    /// Returns true if the view has changed in such a way that all specialized
    /// pipelines for entities visible from it must be regenerated.
    pub fn must_wipe_specializations_for_view(&self, view: RetainedViewEntity) -> bool {
        self.views.contains(&view)
    }

    /// Given a main entity known to be visible, returns it alongside any render
    /// entity it corresponds to.
    ///
    /// If no render entity corresponds to the given main entity, the render
    /// entity returned will be [`Entity::PLACEHOLDER`].
    fn entity_pair_from_visible_main_entity<'a>(
        &'a self,
        render_visible_mesh_entities: &'a RenderVisibleEntitiesClass,
        main_entity: &'a MainEntity,
    ) -> Option<(&'a Entity, &'a MainEntity)> {
        // Check entities with CPU culling.
        if let Ok(index) = render_visible_mesh_entities
            .entities_cpu_culling
            .binary_search_by_key(main_entity, |(_, main_entity)| *main_entity)
        {
            let (key, value) = &render_visible_mesh_entities.entities_cpu_culling[index];
            return Some((key, value));
        }

        // Check entities that opted out of CPU culling.
        if let Some(entity) = render_visible_mesh_entities
            .entities_gpu_culling
            .get(main_entity)
        {
            return Some((entity, main_entity));
        }

        // We didn't find the entity, so return `None`.
        None
    }

    /// Iterates over all entities that need their specializations cleared in
    /// this frame.
    pub fn iter_to_despecialize<'a>(&'a self) -> impl Iterator<Item = &'a MainEntity> {
        // Entities that changed or were removed must be
        // de-specialized.
        self.changed_renderables
            .iter()
            .chain(self.removed_renderables.iter())
    }

    /// Iterates over all entities that need to have their pipelines
    /// re-specialized this frame.
    ///
    /// `last_frame_view_pending_queues` should be the contents of the
    /// [`ViewPendingQueues::prev_frame`] list.
    pub fn iter_to_specialize<'a>(
        &'a self,
        view: RetainedViewEntity,
        render_view_visible_mesh_entities: &'a RenderVisibleEntitiesClass,
        last_frame_view_pending_queues: &'a HashSet<(Entity, MainEntity)>,
    ) -> impl Iterator<Item = (&'a Entity, &'a MainEntity)> {
        (if self.must_wipe_specializations_for_view(view) {
            Either::Left(render_view_visible_mesh_entities.iter_visible())
        } else {
            Either::Right(
                render_view_visible_mesh_entities
                    .added_entities()
                    .iter()
                    .map(|(entity, main_entity)| (entity, main_entity))
                    .chain(self.changed_renderables.iter().filter_map(|main_entity| {
                        self.entity_pair_from_visible_main_entity(
                            render_view_visible_mesh_entities,
                            main_entity,
                        )
                    })),
            )
        })
        .chain(last_frame_view_pending_queues.iter().filter_map(
            |(entity, main_entity)| {
                if render_view_visible_mesh_entities.entity_pair_is_visible(*entity, *main_entity) {
                    Some((entity, main_entity))
                } else {
                    None
                }
            },
        ))
    }

    /// Iterates over all renderables that should be removed from the phase.
    ///
    /// This includes renderables that became invisible this frame, renderables
    /// that are in [`DirtySpecializations::changed_renderables`], and
    /// renderables that are in [`DirtySpecializations::removed_renderables`].
    /// If this view must itself be re-specialized, this will iterate over all
    /// visible entities in addition to those that became invisible.
    pub fn iter_to_dequeue<'a>(
        &'a self,
        view: RetainedViewEntity,
        render_visible_mesh_entities: &'a RenderVisibleEntitiesClass,
    ) -> impl Iterator<Item = &'a MainEntity> {
        render_visible_mesh_entities
            .removed_entities
            .iter()
            .map(|(_, main_entity)| main_entity)
            .chain(if self.must_wipe_specializations_for_view(view) {
                // All visible entities must be removed.
                // Note that this includes potentially-invisible entities, but
                // that's OK as they shouldn't be in the caller's bins in the
                // first place.
                Either::Left(
                    render_visible_mesh_entities
                        .iter_visible()
                        .map(|(_, main_entity)| main_entity),
                )
            } else {
                // Only entities that changed must be removed.
                Either::Right(
                    self.changed_renderables
                        .iter()
                        .chain(self.removed_renderables.iter()),
                )
            })
    }

    /// Iterates over all renderables that potentially need to be re-queued.
    ///
    /// This includes both renderables that became visible and those that are in
    /// [`DirtySpecializations::changed_renderables`]. If this view must itself
    /// be re-specialized, this will iterate over all visible renderables.
    ///
    /// `last_frame_view_pending_queues` should be the contents of the
    /// [`ViewPendingQueues::prev_frame`] list.
    pub fn iter_to_queue<'a>(
        &'a self,
        view: RetainedViewEntity,
        render_visible_mesh_entities: &'a RenderVisibleEntitiesClass,
        last_frame_view_pending_queues: &'a HashSet<(Entity, MainEntity)>,
    ) -> impl Iterator<Item = (&'a Entity, &'a MainEntity)> {
        (if self.must_wipe_specializations_for_view(view) {
            Either::Left(render_visible_mesh_entities.iter_visible())
        } else {
            Either::Right(
                render_visible_mesh_entities
                    .added_entities()
                    .iter()
                    .map(|(entity, main_entity)| (entity, main_entity))
                    .chain(self.changed_renderables.iter().filter_map(|main_entity| {
                        // Only include entities that need respecialization, are
                        // visible, and *didn't* become visible this frame. The
                        // third criterion exists because we already yielded
                        // such entities just prior to this and don't want to
                        // yield the same entity twice.
                        // Note that binary searching works because all lists in
                        // `RenderVisibleEntities` are guaranteed to be sorted.
                        if render_visible_mesh_entities
                            .added_entities()
                            .binary_search_by_key(main_entity, |(_, main_entity)| *main_entity)
                            .is_err()
                        {
                            self.entity_pair_from_visible_main_entity(
                                render_visible_mesh_entities,
                                main_entity,
                            )
                        } else {
                            None
                        }
                    })),
            )
        })
        .chain(last_frame_view_pending_queues.iter().filter_map(
            |(entity, main_entity)| {
                if render_visible_mesh_entities.entity_pair_is_visible(*entity, *main_entity) {
                    Some((entity, main_entity))
                } else {
                    None
                }
            },
        ))
    }
}
