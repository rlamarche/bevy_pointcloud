use bevy::{
    ecs::entity::Entity,
    material::{descriptor::CachedRenderPipelineId, labels::DrawFunctionId},
    platform::collections::hash_map::Entry,
    render::{
        mesh::allocator::MeshSlabs,
        render_phase::{BinnedPhaseItem, BinnedRenderPhase, PhaseItemBatchSetKey},
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

/// Information that must be identical in order to place opaque meshes in the
/// same *batch set*.
///
/// A batch set is a set of batches that can be multi-drawn together, if
/// multi-draw is in use.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkOpaque3dBatchSetKey {
    /// The identifier of the render pipeline.
    pub pipeline: CachedRenderPipelineId,

    /// The function used to draw.
    pub draw_function: DrawFunctionId,

    /// The ID of a bind group specific to the material instance.
    ///
    /// In the case of PBR, this is the `MaterialBindGroupIndex`.
    pub material_bind_group_index: Option<u32>,

    pub splat_slabs: MeshSlabs,

    /// The IDs of the slabs of GPU memory in the mesh allocator that contain
    /// the mesh data.
    ///
    /// For non-mesh items, you can fill the [`MeshSlabs::vertex_slab_id`] with
    /// 0 if your items can be multi-drawn, or with a unique value if they
    /// can't.
    pub slabs: MeshSlabs,
}

impl PhaseItemBatchSetKey for ChunkOpaque3dBatchSetKey {
    fn indexed(&self) -> bool {
        self.splat_slabs.index_slab_id.is_some()
    }
}
