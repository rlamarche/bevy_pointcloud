pub mod batching;

use std::{marker::PhantomData, ops::Range};

use bevy::{
    app::{App, Plugin},
    ecs::{
        entity::{Entity, EntityHashMap},
        resource::Resource,
        schedule::IntoScheduleConfigs,
        world::World,
    },
    platform::collections::{hash_map::Entry, HashMap},
    prelude::{Deref, DerefMut},
    render::{
        batching::gpu_preprocessing::GpuPreprocessingMode,
        render_phase::{
            BinnedPhaseItem, DrawError, DrawFunctions, PhaseItemExtraIndex, TrackedRenderPass,
        },
        sync_world::{MainEntity, MainEntityHashMap},
        view::RetainedViewEntity,
        GpuResourceAppExt, Render, RenderApp, RenderDebugFlags, RenderSystems,
    },
    utils::default,
};
use indexmap::IndexMap;
use smallvec::SmallVec;

use crate::render::render_phase::batching::{
    clear_indirect_parameters_buffers, GetBatchData, PhaseIndirectParametersBuffers,
};

#[derive(Resource, Deref, DerefMut)]
pub struct ViewBinnedChunksRenderPhases<BPI>(
    pub HashMap<RetainedViewEntity, BinnedChunksRenderPhase<BPI>>,
)
where
    BPI: BinnedPhaseItem;

pub struct BinnedChunksRenderPhase<BPI>
where
    BPI: BinnedPhaseItem,
{
    /// Items in the bin that aren't meshes at all.
    ///
    /// Bevy itself doesn't place anything in this list, but plugins or your app
    /// can in order to execute custom drawing commands. Draw functions for each
    /// entity are simply called in order at rendering time.
    ///
    /// See the `custom_phase_item` example for an example of how to use this.
    pub chunk_items: IndexMap<BPI::BatchSetKey, RenderMultidrawableChunks<BPI>>,

    /// Information on each batch set.
    ///
    /// A *batch set* is a set of entities that will be batched together unless
    /// we're on a platform that doesn't support storage buffers (e.g. WebGL 2)
    /// and differing dynamic uniform indices force us to break batches. On
    /// platforms that support storage buffers, a batch set always consists of
    /// at most one batch.
    ///
    /// Multidrawable entities come first, then batchable entities, then
    /// unbatchable entities.
    pub(crate) batch_sets: BinnedChunksRenderPhaseBatchSets<BPI::BinKey>,

    /// The batch and bin key for each entity.
    ///
    /// We retain these so that, when the entity changes, the methods that
    /// remove items from bins can quickly find the bin each entity was located
    /// in in order to remove it.
    cached_entity_bin_keys: MainEntityHashMap<EntityHashMap<CachedBinnedEntity<BPI>>>,

    /// The gpu preprocessing mode configured for the view this phase is associated
    /// with.
    gpu_preprocessing_mode: GpuPreprocessingMode,
}

/// Information about [`BinnedRenderPhaseType::NonMesh`] entities.
pub struct RenderMultidrawableChunks<BPI>
where
    BPI: BinnedPhaseItem,
{
    pub(crate) entities: HashMap<BPI::BinKey, RenderMultidrawableChunk>,
    /// The range in the indirect buffer
    pub(crate) range: Option<Range<u32>>,
}

impl<BPI> RenderMultidrawableChunks<BPI> where BPI: BinnedPhaseItem {}

#[derive(Clone)]
pub struct RenderMultidrawableChunk {
    pub representative_entity: (Entity, MainEntity),
}

impl<BPI> Default for ViewBinnedChunksRenderPhases<BPI>
where
    BPI: BinnedPhaseItem,
{
    fn default() -> Self {
        Self(default())
    }
}

impl<BPI> ViewBinnedChunksRenderPhases<BPI>
where
    BPI: BinnedPhaseItem,
{
    pub fn prepare_for_new_frame(
        &mut self,
        retained_view_entity: RetainedViewEntity,
        gpu_preprocessing: GpuPreprocessingMode,
        no_indirect_drawing: bool,
    ) {
        match self.entry(retained_view_entity) {
            Entry::Occupied(mut entry) => entry.get_mut().prepare_for_new_frame(),
            Entry::Vacant(entry) => {
                entry.insert(BinnedChunksRenderPhase::<BPI>::new(
                    gpu_preprocessing,
                    no_indirect_drawing,
                ));
            }
        }
    }
}

/// Information that we keep about an entity currently within a bin.
pub struct CachedBinnedEntity<BPI>
where
    BPI: BinnedPhaseItem,
{
    /// Information that we use to identify a cached entity in a bin.
    pub cached_bin_key: Option<CachedBinKey<BPI>>,
}

/// Information that we use to identify a cached entity in a bin.
pub struct CachedBinKey<BPI>
where
    BPI: BinnedPhaseItem,
{
    /// The key of the batch set containing the entity.
    pub batch_set_key: BPI::BatchSetKey,
    /// The key of the bin containing the entity.
    pub bin_key: BPI::BinKey,
}

impl<BPI> Clone for CachedBinnedEntity<BPI>
where
    BPI: BinnedPhaseItem,
{
    fn clone(&self) -> Self {
        CachedBinnedEntity {
            cached_bin_key: self.cached_bin_key.clone(),
        }
    }
}

impl<BPI> Clone for CachedBinKey<BPI>
where
    BPI: BinnedPhaseItem,
{
    fn clone(&self) -> Self {
        CachedBinKey {
            batch_set_key: self.batch_set_key.clone(),
            bin_key: self.bin_key.clone(),
        }
    }
}

impl<BPI> PartialEq for CachedBinKey<BPI>
where
    BPI: BinnedPhaseItem,
{
    fn eq(&self, other: &Self) -> bool {
        self.batch_set_key == other.batch_set_key && self.bin_key == other.bin_key
    }
}

/// How we store and render the batch sets.
///
/// Each one of these corresponds to a [`GpuPreprocessingMode`].
pub enum BinnedChunksRenderPhaseBatchSets<BK> {
    /// Batches are never grouped into batch sets.
    ///
    /// This is where `multi_draw_indirect` is not supported.
    Direct(Vec<SmallVec<[BinnedChunksRenderPhaseBatch<BK>; 1]>>),

    /// Batches are grouped together into batch sets based on their ability to
    /// be multi-drawn together.
    ///
    /// This is where `multi_draw_indirect` is supported.
    MultidrawIndirect(Vec<BinnedChunksRenderPhaseBatchSet<BK>>),
}

/// A group of entities that will be batched together into a single multi-draw
/// call.
pub struct BinnedChunksRenderPhaseBatchSet<BK> {
    /// The first batch in this batch set.
    pub(crate) first_batch: BinnedChunksRenderPhaseBatch<BK>,
    /// The number of batches.
    pub(crate) batch_count: u32,
}

impl<BK> BinnedChunksRenderPhaseBatchSets<BK> {
    fn clear(&mut self) {
        match *self {
            BinnedChunksRenderPhaseBatchSets::Direct(ref mut vec) => vec.clear(),
            BinnedChunksRenderPhaseBatchSets::MultidrawIndirect(ref mut vec) => vec.clear(),
        }
    }
}

/// Information about a single batch of entities rendered using binned phase
/// items.
#[derive(Debug)]
pub struct BinnedChunksRenderPhaseBatch<BK> {
    /// An entity that's *representative* of this batch.
    ///
    /// Bevy uses this to fetch the mesh. It can be any entity in the batch.
    pub representative_entity: (Entity, MainEntity),
    pub bin_key: BK,
    pub extra_index: PhaseItemExtraIndex,
}

impl<BPI> BinnedChunksRenderPhase<BPI>
where
    BPI: BinnedPhaseItem,
{
    /// Bins a new entity.
    pub fn add(
        &mut self,
        batch_set_key: BPI::BatchSetKey,
        bin_key: BPI::BinKey,
        (entity, main_entity): (Entity, MainEntity),
    ) {
        match self.chunk_items.entry(batch_set_key.clone()) {
            indexmap::map::Entry::Occupied(mut entry) => {
                entry.get_mut().entities.insert(
                    bin_key.clone(),
                    RenderMultidrawableChunk {
                        representative_entity: (entity, main_entity),
                    },
                );
            }
            indexmap::map::Entry::Vacant(entry) => {
                let mut entities = HashMap::default();
                entities.insert(
                    bin_key.clone(),
                    RenderMultidrawableChunk {
                        representative_entity: (entity, main_entity),
                    },
                );
                entry.insert(RenderMultidrawableChunks {
                    entities,
                    range: None,
                });
            }
        }

        // Update the cache.
        self.update_cache(
            entity,
            main_entity,
            Some(CachedBinKey {
                batch_set_key,
                bin_key,
            }),
        );
    }

    /// Inserts an entity into the cache with the given change tick.
    pub fn update_cache(
        &mut self,
        entity: Entity,
        main_entity: MainEntity,
        cached_bin_key: Option<CachedBinKey<BPI>>,
    ) {
        let new_cached_binned_entity = CachedBinnedEntity { cached_bin_key };
        self.cached_entity_bin_keys
            .entry(main_entity)
            .or_default()
            .insert(entity, new_cached_binned_entity);
    }

    /// Encodes the GPU commands needed to render all entities in this phase.
    pub fn render<'w>(
        &self,
        render_pass: &mut TrackedRenderPass<'w>,
        world: &'w World,
        view: Entity,
    ) -> Result<(), DrawError> {
        {
            let draw_functions = world.resource::<DrawFunctions<BPI>>();
            let mut draw_functions = draw_functions.write();
            draw_functions.prepare(world);
            // Make sure to drop the reader-writer lock here to avoid recursive
            // locks.
        }

        self.render_chunks(render_pass, world, view)?;

        Ok(())
    }

    /// Renders all batchable meshes queued in this phase.
    fn render_chunks<'w>(
        &self,
        render_pass: &mut TrackedRenderPass<'w>,
        world: &'w World,
        view: Entity,
    ) -> Result<(), DrawError> {
        let draw_functions = world.resource::<DrawFunctions<BPI>>();
        let mut draw_functions = draw_functions.write();

        // let render_device = world.resource::<RenderDevice>();
        // let render_adapter_info = world.resource::<RenderAdapterInfo>();
        // let multi_draw_indirect_count_supported = render_device
        //     .features()
        //     .contains(Features::MULTI_DRAW_INDIRECT_COUNT)
        //     // TODO: https://github.com/gfx-rs/wgpu/issues/7974
        //     && !matches!(render_adapter_info.backend, wgpu::Backend::Dx12);

        match self.batch_sets {
            BinnedChunksRenderPhaseBatchSets::Direct(ref batch_sets) => {
                for (batch_set_key, batch_set) in self.chunk_items.keys().zip(batch_sets.iter()) {
                    for batch in batch_set {
                        let binned_phase_item = BPI::new(
                            batch_set_key.clone(),
                            batch.bin_key.clone(),
                            batch.representative_entity,
                            // will not be used because the range will be get in the draw function
                            // itself
                            0..0,
                            PhaseItemExtraIndex::None,
                        );

                        // Fetch the draw function.
                        let Some(draw_function) =
                            draw_functions.get_mut(binned_phase_item.draw_function())
                        else {
                            continue;
                        };

                        draw_function.draw(world, render_pass, view, &binned_phase_item)?;
                    }
                }
            }

            BinnedChunksRenderPhaseBatchSets::MultidrawIndirect(ref batch_sets) => {
                for (batch_set_key, batch_set) in self.chunk_items.keys().zip(batch_sets.iter()) {
                    let batch = &batch_set.first_batch;

                    let binned_phase_item = BPI::new(
                        batch_set_key.clone(),
                        batch.bin_key.clone(),
                        batch.representative_entity,
                        // will not be used because the range is passed through indirect buffers
                        0..0,
                        match batch.extra_index {
                            PhaseItemExtraIndex::IndirectParametersIndex { ref range, .. } => {
                                PhaseItemExtraIndex::IndirectParametersIndex {
                                    range: range.start..(range.start + batch_set.batch_count),
                                    batch_set_index: None,
                                }
                            }
                            _ => panic!("invalid phase item extra index"),
                        },
                    );

                    // Fetch the draw function.
                    let Some(draw_function) =
                        draw_functions.get_mut(binned_phase_item.draw_function())
                    else {
                        continue;
                    };

                    draw_function.draw(world, render_pass, view, &binned_phase_item)?;
                }
            }
        }

        Ok(())
    }

    //     /// Renders all objects of type [`BinnedRenderPhaseType::NonMesh`].
    //     ///
    //     /// These will have been added by plugins or the application.
    //     fn render_non_meshes<'w>(
    //         &self,
    //         render_pass: &mut TrackedRenderPass<'w>,
    //         world: &'w World,
    //         view: Entity,
    //     ) -> Result<(), DrawError> {
    //         let draw_functions = world.resource::<DrawFunctions<BPI>>();
    //         let mut draw_functions = draw_functions.write();
    //
    //         for ((batch_set_key, bin_key), non_mesh_entities) in &self.chunk_items {
    //             for (main_entity, entity) in non_mesh_entities.entities.iter() {
    //                 // Come up with a fake batch range and extra index. The draw
    //                 // function is expected to manage any sort of batching logic itself.
    //                 let binned_phase_item = BPI::new(
    //                     batch_set_key.clone(),
    //                     bin_key.clone(),
    //                     (*entity, *main_entity),
    //                     0..1,
    //                     PhaseItemExtraIndex::None,
    //                 );
    //
    //                 let Some(draw_function) =
    // draw_functions.get_mut(binned_phase_item.draw_function())                 else {
    //                     continue;
    //                 };
    //
    //                 draw_function.draw(world, render_pass, view, &binned_phase_item)?;
    //             }
    //         }
    //
    //         Ok(())
    //     }

    pub fn is_empty(&self) -> bool {
        self.chunk_items.is_empty()
    }

    pub fn prepare_for_new_frame(&mut self) {
        self.batch_sets.clear();
    }

    /// Removes a single entity from its bin.
    ///
    /// If doing so makes the bin empty, this method removes the bin as well.
    pub fn remove(&mut self, entity: Entity, main_entity: MainEntity) {
        let Some(cached_binned_entity) = self
            .cached_entity_bin_keys
            .get_mut(&main_entity)
            .and_then(|cache| cache.remove(&entity))
        else {
            return;
        };

        if let Some(ref cached_bin_key) = cached_binned_entity.cached_bin_key {
            remove_entity_from_bin(cached_bin_key, &mut self.chunk_items);
        }
    }
}

fn remove_entity_from_bin<BPI>(
    entity_bin_key: &CachedBinKey<BPI>,
    chunk_items: &mut IndexMap<BPI::BatchSetKey, RenderMultidrawableChunks<BPI>>,
) where
    BPI: BinnedPhaseItem,
{
    if let indexmap::map::Entry::Occupied(mut bin_entry) =
        chunk_items.entry(entity_bin_key.batch_set_key.clone())
    {
        bin_entry.get_mut().entities.remove(&entity_bin_key.bin_key);

        // If the bin is now empty, remove the bin.
        if bin_entry.get_mut().entities.is_empty() {
            bin_entry.swap_remove();
        }
    }
}

impl<BPI> BinnedChunksRenderPhase<BPI>
where
    BPI: BinnedPhaseItem,
{
    fn new(gpu_preprocessing: GpuPreprocessingMode, no_indirect_drawing: bool) -> Self {
        Self {
            chunk_items: IndexMap::default(),
            batch_sets: match gpu_preprocessing {
                GpuPreprocessingMode::Culling if !no_indirect_drawing => {
                    BinnedChunksRenderPhaseBatchSets::MultidrawIndirect(vec![])
                }
                _ => BinnedChunksRenderPhaseBatchSets::Direct(vec![]),
            },
            cached_entity_bin_keys: MainEntityHashMap::default(),
            gpu_preprocessing_mode: gpu_preprocessing,
        }
    }
}

pub struct BinnedChunksRenderPhasePlugin<BPI, GFBD>
where
    BPI: BinnedPhaseItem,
    GFBD: GetBatchData,
{
    /// Debugging flags that can optionally be set when constructing the renderer.
    pub debug_flags: RenderDebugFlags,
    phantom: PhantomData<(BPI, GFBD)>,
}

impl<BPI, GFBD> BinnedChunksRenderPhasePlugin<BPI, GFBD>
where
    BPI: BinnedPhaseItem,
    GFBD: GetBatchData,
{
    pub fn new(debug_flags: RenderDebugFlags) -> Self {
        Self {
            debug_flags,
            phantom: PhantomData,
        }
    }
}

impl<BPI, GFBD> Plugin for BinnedChunksRenderPhasePlugin<BPI, GFBD>
where
    BPI: BinnedPhaseItem,
    GFBD: GetBatchData + Sync + Send + 'static,
{
    fn build(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_gpu_resource::<ViewBinnedChunksRenderPhases<BPI>>()
            .allow_ambiguous_resource::<ViewBinnedChunksRenderPhases<BPI>>()
            .init_gpu_resource::<PhaseIndirectParametersBuffers<BPI>>()
            .add_systems(
                Render,
                (
                    batching::sort_binned_chunks_render_phase::<BPI>
                        .in_set(RenderSystems::PhaseSort),
                    batching::batch_and_prepare_binned_render_phase::<BPI, GFBD>
                        .in_set(RenderSystems::PrepareResourcesBatchPhases)
                        .ambiguous_with(RenderSystems::PrepareResourcesBatchPhases),
                    batching::write_indirect_parameters_buffers::<BPI>
                        .in_set(RenderSystems::PrepareResourcesWritePhaseBuffers)
                        .ambiguous_with(RenderSystems::PrepareResourcesWritePhaseBuffers),
                    clear_indirect_parameters_buffers::<BPI>.in_set(RenderSystems::PrepareViews),
                ),
            );
    }
}
