use std::marker::PhantomData;

use bevy::{
    ecs::{
        entity::Entity,
        query::{Has, With},
        resource::Resource,
        system::{Query, Res, ResMut, StaticSystemParam, SystemParam, SystemParamItem},
        world::{FromWorld, World},
    },
    log::warn,
    prelude::{Deref, DerefMut},
    render::{
        batching::gpu_preprocessing::{
            GpuPreprocessingMode, IndirectParametersBuffersSettings, IndirectParametersIndexed,
            IndirectParametersNonIndexed,
        },
        render_phase::{
            BinnedPhaseItem, PhaseItem, PhaseItemBatchSetKey as _, PhaseItemExtraIndex,
        },
        render_resource::{
            encase::private::WriteInto, Buffer, BufferUsages, BufferVec, ShaderSize,
        },
        renderer::{RenderDevice, RenderQueue},
        sync_world::MainEntity,
        view::{ExtractedView, NoIndirectDrawing},
    },
};
use smallvec::SmallVec;

use crate::render::{
    render_phase::{
        BinnedChunksRenderPhaseBatch, BinnedChunksRenderPhaseBatchSet,
        BinnedChunksRenderPhaseBatchSets, RenderMultidrawableChunk, ViewBinnedChunksRenderPhases,
    },
    IndirectParameters,
};

pub trait GetBatchData {
    type Param: SystemParam + 'static;

    fn get_indirect_parameters_indexed(
        param: &SystemParamItem<Self::Param>,
        query_item: (Entity, MainEntity),
    ) -> Option<IndirectParametersIndexed>;

    /// Returns the instances range to render from the instances buffer for a given entity
    fn get_indirect_parameters_non_indexed(
        param: &SystemParamItem<Self::Param>,
        query_item: (Entity, MainEntity),
    ) -> Option<IndirectParametersNonIndexed>;
}

/// Sorts a render phase that uses bins.
pub fn sort_binned_chunks_render_phase<BPI>(mut phases: ResMut<ViewBinnedChunksRenderPhases<BPI>>)
where
    BPI: BinnedPhaseItem,
{
    for phase in phases.values_mut() {
        phase.chunk_items.sort_unstable_keys();
        for chunk_item in phase.chunk_items.values_mut() {
            // this sorts chunk nodes from root to leaf
            chunk_item.entities.sort_unstable_keys();
        }
    }
}

/// Creates batches for a render phase that uses bins.
pub fn batch_and_prepare_binned_render_phase<BPI, GFBD>(
    phase_indirect_parameters_buffers: ResMut<PhaseIndirectParametersBuffers<BPI>>,
    mut binned_render_phases: ResMut<ViewBinnedChunksRenderPhases<BPI>>,
    mut views: Query<(&ExtractedView, Has<NoIndirectDrawing>), With<ExtractedView>>,
    param: StaticSystemParam<GFBD::Param>,
) where
    BPI: BinnedPhaseItem,
    GFBD: GetBatchData,
{
    let system_param_item = param.into_inner();

    let phase_indirect_parameters_buffers = phase_indirect_parameters_buffers.into_inner();

    for (extracted_view, no_indirect_drawing) in &mut views {
        let Some(phase) = binned_render_phases.get_mut(&extracted_view.retained_view_entity) else {
            continue;
        };

        // check if multi draw indirect is activated or not
        let multi_draw_indirect =
            !no_indirect_drawing && phase.gpu_preprocessing_mode == GpuPreprocessingMode::Culling;

        // Prepare unbatchables.
        for (key, chunks) in &mut phase.chunk_items {
            let indirect_parameters_offset = if multi_draw_indirect {
                if key.indexed() {
                    Some(
                        phase_indirect_parameters_buffers
                            .buffers
                            .indexed
                            .indirect_draw_parameters
                            .len() as u32,
                    )
                } else {
                    Some(
                        phase_indirect_parameters_buffers
                            .buffers
                            .non_indexed
                            .indirect_draw_parameters
                            .len() as u32,
                    )
                }
            } else {
                None
            };

            let mut batch: Option<BinnedChunksRenderPhaseBatch<BPI::BinKey>> = None;
            let mut batch_count: u32 = 0;

            let mut batches = None;
            if let BinnedChunksRenderPhaseBatchSets::Direct(_) = phase.batch_sets {
                batches = Some(SmallVec::new());
            }

            for (
                bin_key,
                RenderMultidrawableChunk {
                    representative_entity,
                    indirect_parameters,
                },
            ) in chunks.entities.iter_mut()
            {
                if multi_draw_indirect {
                    if key.indexed() {
                        if let Some(indirect_parameters_indexed) = {
                            match indirect_parameters {
                                Some(IndirectParameters::Indexed(indirect_parameters_indexed)) => {
                                    Some(*indirect_parameters_indexed)
                                }
                                _ => {
                                    if let Some(indirect_parameters_indexed) =
                                        GFBD::get_indirect_parameters_indexed(
                                            &system_param_item,
                                            *representative_entity,
                                        )
                                    {
                                        *indirect_parameters = Some(IndirectParameters::Indexed(
                                            indirect_parameters_indexed,
                                        ));
                                        Some(indirect_parameters_indexed)
                                    } else {
                                        None
                                    }
                                }
                            }
                        } {
                            phase_indirect_parameters_buffers
                                .buffers
                                .add_batch_set_indexed(indirect_parameters_indexed);
                            batch_count += 1;
                        } else {
                            warn!("Missing indirect parameters indexed");
                        }
                    } else {
                        if let Some(indirect_parameters_non_indexed) = {
                            match indirect_parameters {
                                Some(IndirectParameters::NonIndexed(
                                    indirect_parameters_non_indexed,
                                )) => Some(*indirect_parameters_non_indexed),
                                _ => {
                                    if let Some(indirect_parameters_non_indexed) =
                                        GFBD::get_indirect_parameters_non_indexed(
                                            &system_param_item,
                                            *representative_entity,
                                        )
                                    {
                                        *indirect_parameters =
                                            Some(IndirectParameters::NonIndexed(
                                                indirect_parameters_non_indexed,
                                            ));
                                        Some(indirect_parameters_non_indexed)
                                    } else {
                                        None
                                    }
                                }
                            }
                        } {
                            phase_indirect_parameters_buffers
                                .buffers
                                .add_batch_set_non_indexed(indirect_parameters_non_indexed);
                            batch_count += 1;
                        } else {
                            warn!("Missing indirect parameters non indexed");
                        }
                    }

                    if batch.is_none() {
                        batch = Some(BinnedChunksRenderPhaseBatch {
                            representative_entity: *representative_entity,
                            bin_key: bin_key.clone(),
                            // will be set at the end
                            extra_index: PhaseItemExtraIndex::None,
                        });
                    }
                } else {
                    if let Some(ref mut batches) = batches {
                        batches.push(BinnedChunksRenderPhaseBatch {
                            representative_entity: *representative_entity,
                            bin_key: bin_key.clone(),
                            extra_index: PhaseItemExtraIndex::None,
                        });
                    }
                }
            }

            match phase.batch_sets {
                BinnedChunksRenderPhaseBatchSets::Direct(ref mut vec) => {
                    if let Some(batches) = batches {
                        vec.push(batches);
                    }
                }
                BinnedChunksRenderPhaseBatchSets::MultidrawIndirect(ref mut vec) => {
                    let Some(indirect_parameters_offset) = indirect_parameters_offset else {
                        continue;
                    };

                    chunks.range =
                        Some(indirect_parameters_offset..indirect_parameters_offset + batch_count);

                    if let Some(mut batch) = batch {
                        batch.extra_index = PhaseItemExtraIndex::IndirectParametersIndex {
                            range: indirect_parameters_offset
                                ..indirect_parameters_offset + batch_count,
                            batch_set_index: None,
                        };
                        vec.push(BinnedChunksRenderPhaseBatchSet {
                            first_batch: batch,
                            batch_count,
                        });
                    }
                }
            }
        }
    }
}

/// The buffers containing all the information that indirect draw commands use
/// to draw the scene, for a single phase.
///
/// This is the version of the structure that has a type parameter, so that the
/// batching for different phases can run in parallel.
///
/// See the [`IndirectParametersBuffers`] documentation for more information.
#[derive(Resource, Deref, DerefMut)]
pub struct PhaseIndirectParametersBuffers<PI>
where
    PI: PhaseItem,
{
    #[deref]
    /// The indirect draw buffers for the phase.
    pub buffers: UntypedPhaseIndirectParametersBuffers,
    phantom: PhantomData<PI>,
}

impl<PI> FromWorld for PhaseIndirectParametersBuffers<PI>
where
    PI: PhaseItem,
{
    fn from_world(world: &mut World) -> Self {
        let settings = world.resource::<IndirectParametersBuffersSettings>();
        PhaseIndirectParametersBuffers {
            buffers: UntypedPhaseIndirectParametersBuffers::new(
                settings.allow_copies_from_indirect_parameter_buffers,
            ),
            phantom: PhantomData,
        }
    }
}

/// The buffers containing all the information that indirect draw commands use
/// to draw the scene, for a single phase.
///
/// This is the version of the structure that doesn't have a type parameter, so
/// that it can be inserted into [`IndirectParametersBuffers::buffers`]
///
/// See the [`IndirectParametersBuffers`] documentation for more information.
pub struct UntypedPhaseIndirectParametersBuffers {
    /// Information that indirect draw commands use to draw indexed meshes in
    /// the scene.
    pub indexed: MeshClassIndirectParametersBuffers<IndirectParametersIndexed>,
    /// Information that indirect draw commands use to draw non-indexed meshes
    /// in the scene.
    pub non_indexed: MeshClassIndirectParametersBuffers<IndirectParametersNonIndexed>,
}

impl UntypedPhaseIndirectParametersBuffers {
    /// Creates the indirect parameters buffers.
    pub fn new(
        allow_copies_from_indirect_parameter_buffers: bool,
    ) -> UntypedPhaseIndirectParametersBuffers {
        UntypedPhaseIndirectParametersBuffers {
            non_indexed: MeshClassIndirectParametersBuffers::new(
                allow_copies_from_indirect_parameter_buffers,
            ),
            indexed: MeshClassIndirectParametersBuffers::new(
                allow_copies_from_indirect_parameter_buffers,
            ),
        }
    }

    #[inline]
    pub fn add_batch_set_indexed(
        &mut self,
        indirect_parameters_indexed: IndirectParametersIndexed,
    ) {
        self.indexed
            .indirect_draw_parameters
            .push(indirect_parameters_indexed);
    }

    #[inline]
    pub fn add_batch_set_non_indexed(
        &mut self,
        indirect_parameters_non_indexed: IndirectParametersNonIndexed,
    ) {
        self.non_indexed
            .indirect_draw_parameters
            .push(indirect_parameters_non_indexed);
    }

    /// Clears out the buffers in preparation for a new frame.
    pub fn clear(&mut self) {
        self.indexed.clear();
        self.non_indexed.clear();
    }
}

/// The buffers containing all the information that indirect draw commands use
/// to draw the scene, for a single mesh class (indexed or non-indexed), for a
/// single phase.
pub struct MeshClassIndirectParametersBuffers<IP>
where
    IP: Clone + ShaderSize + WriteInto,
{
    /// The GPU buffer that stores the indirect draw parameters for the meshes.
    ///
    /// The indirect parameters building shader writes to this buffer, while the
    /// `multi_draw_indirect` or `multi_draw_indirect_count` commands read from
    /// it to perform the draws.
    pub indirect_draw_parameters: BufferVec<IP>,
    // /// A mapping from each view to the ranges of batch instances within the
    // /// indirect parameters buffer for that view, render phase, and mesh class.
    // view_to_indirect_parameters_batch_ranges: HashMap<RetainedViewEntity, Vec<Range<u32>>>,
}

/// GPU-side indirect draw parameters for either indexed or non-indexed meshes.
pub trait MeshClassIndirectParameters: Clone + ShaderSize + WriteInto {
    /// Either the string "indexed" or "non-indexed".
    fn debug_label() -> &'static str;
}

impl MeshClassIndirectParameters for IndirectParametersIndexed {
    fn debug_label() -> &'static str {
        "indexed"
    }
}

impl MeshClassIndirectParameters for IndirectParametersNonIndexed {
    fn debug_label() -> &'static str {
        "non-indexed"
    }
}

impl<IP> MeshClassIndirectParametersBuffers<IP>
where
    IP: MeshClassIndirectParameters,
{
    fn new(
        allow_copies_from_indirect_parameter_buffers: bool,
    ) -> MeshClassIndirectParametersBuffers<IP> {
        let mut indirect_parameter_buffer_usages = BufferUsages::STORAGE | BufferUsages::INDIRECT;
        if allow_copies_from_indirect_parameter_buffers {
            indirect_parameter_buffer_usages |= BufferUsages::COPY_SRC;
        }

        MeshClassIndirectParametersBuffers {
            indirect_draw_parameters: BufferVec::new(indirect_parameter_buffer_usages),
        }
    }

    /// Returns the GPU buffer that stores the indirect draw parameters for
    /// indexed meshes.
    ///
    /// The indirect parameters building shader writes to this buffer, while the
    /// `multi_draw_indirect` or `multi_draw_indirect_count` commands read from
    /// it to perform the draws.
    #[inline]
    pub fn data_buffer(&self) -> Option<&Buffer> {
        self.indirect_draw_parameters.buffer()
    }

    /// Returns the number of batches corresponding to meshes that are currently
    /// allocated.
    #[inline]
    pub fn batch_count(&self) -> usize {
        self.indirect_draw_parameters.len()
    }

    /// Clears out all the buffers in preparation for a new frame.
    pub fn clear(&mut self) {
        self.indirect_draw_parameters.clear();
    }
}

pub fn write_indirect_parameters_buffers<PI>(
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut phase_indirect_parameters_buffers: ResMut<PhaseIndirectParametersBuffers<PI>>,
) where
    PI: PhaseItem,
{
    let render_device = &*render_device;
    let render_queue = &*render_queue;

    phase_indirect_parameters_buffers
        .buffers
        .indexed
        .indirect_draw_parameters
        .write_buffer(render_device, render_queue);

    phase_indirect_parameters_buffers
        .buffers
        .non_indexed
        .indirect_draw_parameters
        .write_buffer(render_device, render_queue);
}

pub fn clear_indirect_parameters_buffers<PI>(
    mut phase_indirect_parameters_buffers: ResMut<PhaseIndirectParametersBuffers<PI>>,
) where
    PI: PhaseItem,
{
    phase_indirect_parameters_buffers.clear();
}
