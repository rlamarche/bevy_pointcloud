use std::any::TypeId;

use bevy::{
    camera::visibility::ViewVisibility,
    ecs::{
        entity::{ContainsEntity, Entity},
        lifecycle::RemovedComponents,
        query::{Changed, Or},
        system::{Query, Res, ResMut},
    },
    log::warn,
    platform::collections::hash_map::Entry,
    render::{
        sync_world::{MainEntity, RenderEntity},
        Extract,
    },
};

use crate::{
    render::{
        EntitiesNeedingSpecialization, PointCloudDirtySpecializations, PointCloudMaterial,
        PreparedPointCloudUniforms, RenderMaterialInstance, RenderPointCloudMaterialInstances,
        ViewPointCloudBindGroups,
    },
    PointCloud3d, PointCloudMaterial3d,
};

/// Fills the [`RenderPointCloudMaterialInstances`] resources from the point clouds in the
/// scene.
pub fn extract_point_cloud_materials<M: PointCloudMaterial>(
    mut material_instances: ResMut<RenderPointCloudMaterialInstances>,
    changed_meshes_query: Extract<
        Query<
            (Entity, &ViewVisibility, &PointCloudMaterial3d<M>),
            Or<(Changed<ViewVisibility>, Changed<PointCloudMaterial3d<M>>)>,
        >,
    >,
) {
    let last_change_tick = material_instances.current_change_tick;

    for (entity, view_visibility, material) in &changed_meshes_query {
        if view_visibility.get() {
            material_instances.instances.insert(
                entity.into(),
                RenderMaterialInstance {
                    asset_id: material.id().untyped(),
                    last_change_tick,
                },
            );
        } else {
            material_instances
                .instances
                .remove(&MainEntity::from(entity));
        }
    }
}

/// Removes point cloud materials from [`RenderPointCloudMaterialInstances`] when their
/// [`PointCloudMaterial3d`] components are removed.
///
/// This is tricky because we have to deal with the case in which a material of
/// type A was removed and replaced with a material of type B in the same frame
/// (which is actually somewhat common of an operation). In this case, even
/// though an entry will be present in `RemovedComponents<PointCloudMaterial3d<A>>`,
/// we must not remove the entry in `RenderMaterialInstances` which corresponds
/// to material B. To handle this case, we use change ticks to avoid removing
/// the entry if it was updated this frame.
///
/// This is the first of two sweep phases. Because this phase runs once per
/// material type, we need a second phase in order to guarantee that we only
/// bump [`RenderPointCloudMaterialInstances::current_change_tick`] once.
pub fn early_sweep_material_instances<M>(
    mut material_instances: ResMut<RenderPointCloudMaterialInstances>,
    mut removed_materials_query: Extract<RemovedComponents<PointCloudMaterial3d<M>>>,
) where
    M: PointCloudMaterial,
{
    let last_change_tick = material_instances.current_change_tick;

    for entity in removed_materials_query.read() {
        if let Entry::Occupied(occupied_entry) = material_instances.instances.entry(entity.into()) {
            // Only sweep the entry if it wasn't updated this frame.
            if occupied_entry.get().last_change_tick != last_change_tick {
                occupied_entry.remove();
            }
        }
    }
}

/// Removes mesh materials from [`RenderPointCloudMaterialInstances`] when their
/// [`ViewVisibility`] components are removed.
///
/// This runs after all invocations of `early_sweep_material_instances` and is
/// responsible for bumping [`RenderPointCloudMaterialInstances::current_change_tick`] in
/// preparation for a new frame.
pub fn late_sweep_material_instances(
    mut material_instances: ResMut<RenderPointCloudMaterialInstances>,
    mut removed_point_clouds_query: Extract<RemovedComponents<PointCloud3d>>,
) {
    let last_change_tick = material_instances.current_change_tick;

    for entity in removed_point_clouds_query.read() {
        if let Entry::Occupied(occupied_entry) = material_instances.instances.entry(entity.into()) {
            // Only sweep the entry if it wasn't updated this frame. It's
            // possible that a `ViewVisibility` component was removed and
            // re-added in the same frame.
            if occupied_entry.get().last_change_tick != last_change_tick {
                occupied_entry.remove();
            }
        }
    }

    material_instances
        .current_change_tick
        .set(last_change_tick.get() + 1);
}

/// Extracts main-world entities requiring pipeline specialization and registers them
/// into the render world's dirty tracking table.
///
/// ### Why it is required
/// In Bevy's rendering architecture, when a point cloud's material flags, layout, or
/// custom attributes change, its associated GPU pipeline may need to be specialized
/// (re-evaluated or recompiled). This system bridges that dirty state between the
/// simulation world and the isolated [`RenderApp`], ensuring the renderer knows which
/// specific entities require pipeline specialization before the draw phase.
pub fn extract_entities_needs_specialization<M>(
    entities_needing_specialization: Extract<Res<EntitiesNeedingSpecialization<M>>>,
    mapper: Extract<Query<&RenderEntity>>,
    mut dirty_specializations: ResMut<PointCloudDirtySpecializations>,
) where
    M: PointCloudMaterial,
{
    // Drain the list of entities needing specialization from the main world
    // into the render-world `DirtySpecializations` table.
    for (entity, render_entity) in entities_needing_specialization.changed.iter() {
        let mut render_entity = *render_entity;
        let main_entity = MainEntity::from(*entity);
        if render_entity.id().eq(&Entity::PLACEHOLDER) {
            render_entity = match mapper.get(*entity) {
                Ok(render_entity) => *render_entity,
                Err(_) => {
                    warn!("Render entity for PointCloud3d {} not found in extract_entities_needs_specialization", entity);
                    continue;
                }
            };
        }

        dirty_specializations
            .changed_renderables
            .insert(render_entity.entity(), main_entity);
    }
}

/// A system that adds entities that were judged to need their specializations
/// removed to the appropriate table in [`DirtySpecializations`].
pub fn extract_entities_that_need_specializations_removed<M>(
    entities_needing_specialization: Extract<Res<EntitiesNeedingSpecialization<M>>>,
    mapper: Extract<Query<&RenderEntity>>,
    mut dirty_specializations: ResMut<PointCloudDirtySpecializations>,
) where
    M: PointCloudMaterial,
{
    for (entity, render_entity) in entities_needing_specialization.removed.iter() {
        let mut render_entity = *render_entity;
        if render_entity.id().eq(&Entity::PLACEHOLDER) {
            render_entity = match mapper.get(*entity) {
                Ok(render_entity) => *render_entity,
                Err(_) => {
                    warn!("Render entity for PointCloud3d {} not found in extract_entities_that_need_specializations_removed", entity);
                    continue;
                }
            };
        }

        dirty_specializations
            .removed_renderables
            .insert(render_entity.entity(), MainEntity::from(*entity));
    }
}

/// Free allocated buffers & bindgroups for removed point cloud's materials
pub fn free_removed_point_cloud_materials<M: PointCloudMaterial>(
    mut removed_items: Extract<RemovedComponents<PointCloudMaterial3d<M>>>,
    mut prepared_point_cloud_uniforms: ResMut<PreparedPointCloudUniforms>,
    mut view_point_clouds_bind_groups: Query<&mut ViewPointCloudBindGroups>,
) {
    let material_type_id = TypeId::of::<M>();
    for entity in removed_items.read() {
        let main_entity = MainEntity::from(entity);
        for mut view_point_cloud_bind_groups in &mut view_point_clouds_bind_groups {
            if let Some(bind_groups) = view_point_cloud_bind_groups
                .bind_groups
                .get_mut(&material_type_id)
            {
                bind_groups.remove(&main_entity);
            }
        }
        prepared_point_cloud_uniforms.remove(&main_entity);
    }
}
