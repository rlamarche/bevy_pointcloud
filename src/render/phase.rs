use bevy::{
    ecs::entity::Entity,
    platform::collections::hash_map::Entry,
    render::{
        render_phase::{BinnedPhaseItem, BinnedRenderPhase},
        sync_world::MainEntity,
    },
};

pub trait BinnedRenderPhaseExt {
    fn remove_unbatchable_entity_pair(&mut self, render_entity: &Entity, main_entity: &MainEntity);

    fn count_items(&self) -> usize;
}

impl<BPI> BinnedRenderPhaseExt for BinnedRenderPhase<BPI>
where
    BPI: BinnedPhaseItem,
{
    /// Removes a single entity from its bin.
    ///
    /// If doing so makes the bin empty, this method removes the bin as well.
    fn remove_unbatchable_entity_pair(&mut self, render_entity: &Entity, main_entity: &MainEntity) {
        let mut keys_to_remove = Vec::new();

        // TODO: create an index for better performances
        for (key, entities) in &mut self.unbatchable_meshes {
            match entities.entities.entry(*main_entity) {
                Entry::Occupied(entry) => {
                    if entry.get().eq(render_entity) {
                        entry.remove();
                    }
                }
                Entry::Vacant(_) => {}
            }
            if entities.entities.is_empty() {
                keys_to_remove.push(key.clone());
            }
        }

        for key in keys_to_remove {
            self.unbatchable_meshes.swap_remove(&key);
        }
    }

    fn count_items(&self) -> usize {
        let mut count = 0;
        for entities in self.unbatchable_meshes.values() {
            count += entities.entities.len();
        }
        count
    }
}
