use bevy::{
    ecs::{entity::Entity, resource::Resource}, platform::collections::HashMap, prelude::{Deref, DerefMut}, render::{camera::PendingQueues, sync_world::MainEntity},
};
use slotmap::{new_key_type, Key, SlotMap};

use crate::PreparedPointCloudUniform;

/// A resource that holds entities that couldn't be specialized and/or queued
///
/// See the documentation of [`PendingQueues`] for more information.
#[derive(Default, Deref, DerefMut, Resource)]
pub struct PendingPointCloudPhaseItemQueues(pub PendingQueues);

new_key_type! { pub struct PointCloudInstanceIndex; }

impl PointCloudInstanceIndex {
    /// Returns the unique index of the point cloud instance, starting at 0
    /// Note: [`slotmap::KeyData::as_ffi`] returns the generation shifted by 32 bits, and index.
    /// Casting to u32 truncates the generation and keep only the index.
    pub fn index(&self) -> u32 {
        self.data().as_ffi() as u32
    }
}

/// This resource stores the point cloud instance mapping to index in render world.
#[derive(Clone, Debug, Default, Resource)]
pub struct RenderPointCloudInstanceIndex {
    pub(crate) slab: SlotMap<PointCloudInstanceIndex, Entity>,
    pub(crate) index: HashMap<Entity, PointCloudInstanceIndex>,
    pub(crate) added: Vec<Entity>,
}

impl RenderPointCloudInstanceIndex {
    /// Add point cloud instance to index, if it already exists, does nothing.
    pub fn add(&mut self, entity: Entity) -> PointCloudInstanceIndex {
        let index = *self
            .index
            .entry(entity)
            // as_ffi returns the generation << 32 + index, casting to u32 truncates the generation
            .or_insert_with(|| self.slab.insert(entity));

        self.added.push(entity);

        index
    }

    /// Removes an entity from the index.
    pub fn remove(&mut self, entity: Entity) -> Option<PointCloudInstanceIndex> {
        if let Some(index) = self.index.remove(&entity) {
            self.slab.remove(index);
            Some(index)
        } else {
            None
        }
    }

    pub fn get(&self, entity: Entity) -> Option<PointCloudInstanceIndex> {
        self.index.get(&entity).copied()
    }
}


#[derive(Resource, Default, Deref, DerefMut)]
pub struct PreparedPointCloudUniforms(HashMap<MainEntity, PreparedPointCloudUniform>);
