use std::ops::Range;

use bevy::{
    camera::{Camera, Camera3d},
    core_pipeline::prepass::{
        DeferredPrepass, DepthPrepass, MotionVectorPrepass, NormalPrepass, OpaqueNoLightmap3dBinKey,
    },
    ecs::{
        entity::Entity,
        query::{Has, With},
        system::{Local, Query, Res, ResMut},
    },
    material::{descriptor::CachedRenderPipelineId, labels::DrawFunctionId},
    platform::collections::HashSet,
    render::{
        batching::gpu_preprocessing::{GpuPreprocessingMode, GpuPreprocessingSupport},
        mesh::allocator::MeshSlabs,
        render_phase::{
            BinnedPhaseItem, CachedRenderPipelinePhaseItem, PhaseItem, PhaseItemBatchSetKey,
            PhaseItemExtraIndex,
        },
        sync_world::MainEntity,
        view::{NoIndirectDrawing, RetainedViewEntity},
        Extract,
    },
};

use crate::render::{
    deferred::{AlphaMask3dChunkDeferred, Opaque3dChunkDeferred},
    render_phase::ViewBinnedChunksRenderPhases,
};

/// Opaque phase of the 3D prepass.
///
/// Sorted by pipeline, then by mesh to improve batching.
///
/// Used to render all 3D meshes with materials that have no transparency.
pub struct Opaque3dChunkPrepass {
    /// Determines which objects can be placed into a *batch set*.
    ///
    /// Objects in a single batch set can potentially be multi-drawn together,
    /// if it's enabled and the current platform supports it.
    pub batch_set_key: OpaqueChunkNoLightmap3dBatchSetKey,
    /// Information that separates items into bins.
    pub bin_key: OpaqueNoLightmap3dBinKey,

    /// An entity from which Bevy fetches data common to all instances in this
    /// batch, such as the mesh.
    pub representative_entity: (Entity, MainEntity),
    pub batch_range: Range<u32>,
    pub extra_index: PhaseItemExtraIndex,
}

/// Information that must be identical in order to place opaque meshes in the
/// same *batch set* in the prepass and deferred pass.
///
/// A batch set is a set of batches that can be multi-drawn together, if
/// multi-draw is in use.
/// TODO: add point cloud entity id to batch per point cloud (because of point cloud uniform
/// binding)
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OpaqueChunkNoLightmap3dBatchSetKey {
    /// The ID of the GPU pipeline.
    pub pipeline: CachedRenderPipelineId,

    /// The function used to draw the mesh.
    pub draw_function: DrawFunctionId,

    /// The ID of a bind group specific to the material.
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

impl PhaseItemBatchSetKey for OpaqueChunkNoLightmap3dBatchSetKey {
    fn indexed(&self) -> bool {
        self.splat_slabs.index_slab_id.is_some()
    }
}

impl PhaseItem for Opaque3dChunkPrepass {
    #[inline]
    fn entity(&self) -> Entity {
        self.representative_entity.0
    }

    fn main_entity(&self) -> MainEntity {
        self.representative_entity.1
    }

    #[inline]
    fn draw_function(&self) -> DrawFunctionId {
        self.batch_set_key.draw_function
    }

    #[inline]
    fn batch_range(&self) -> &Range<u32> {
        &self.batch_range
    }

    #[inline]
    fn batch_range_mut(&mut self) -> &mut Range<u32> {
        &mut self.batch_range
    }

    #[inline]
    fn extra_index(&self) -> PhaseItemExtraIndex {
        self.extra_index.clone()
    }

    #[inline]
    fn batch_range_and_extra_index_mut(&mut self) -> (&mut Range<u32>, &mut PhaseItemExtraIndex) {
        (&mut self.batch_range, &mut self.extra_index)
    }
}

impl BinnedPhaseItem for Opaque3dChunkPrepass {
    type BatchSetKey = OpaqueChunkNoLightmap3dBatchSetKey;
    type BinKey = OpaqueNoLightmap3dBinKey;

    #[inline]
    fn new(
        batch_set_key: Self::BatchSetKey,
        bin_key: Self::BinKey,
        representative_entity: (Entity, MainEntity),
        batch_range: Range<u32>,
        extra_index: PhaseItemExtraIndex,
    ) -> Self {
        Opaque3dChunkPrepass {
            batch_set_key,
            bin_key,
            representative_entity,
            batch_range,
            extra_index,
        }
    }
}

impl CachedRenderPipelinePhaseItem for Opaque3dChunkPrepass {
    #[inline]
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.batch_set_key.pipeline
    }
}

/// Alpha mask phase of the 3D prepass.
///
/// Sorted by pipeline, then by mesh to improve batching.
///
/// Used to render all meshes with a material with an alpha mask.
pub struct AlphaMask3dChunkPrepass {
    /// Determines which objects can be placed into a *batch set*.
    ///
    /// Objects in a single batch set can potentially be multi-drawn together,
    /// if it's enabled and the current platform supports it.
    pub batch_set_key: OpaqueChunkNoLightmap3dBatchSetKey,
    /// Information that separates items into bins.
    pub bin_key: OpaqueNoLightmap3dBinKey,
    pub representative_entity: (Entity, MainEntity),
    pub batch_range: Range<u32>,
    pub extra_index: PhaseItemExtraIndex,
}

impl PhaseItem for AlphaMask3dChunkPrepass {
    #[inline]
    fn entity(&self) -> Entity {
        self.representative_entity.0
    }

    fn main_entity(&self) -> MainEntity {
        self.representative_entity.1
    }

    #[inline]
    fn draw_function(&self) -> DrawFunctionId {
        self.batch_set_key.draw_function
    }

    #[inline]
    fn batch_range(&self) -> &Range<u32> {
        &self.batch_range
    }

    #[inline]
    fn batch_range_mut(&mut self) -> &mut Range<u32> {
        &mut self.batch_range
    }

    #[inline]
    fn extra_index(&self) -> PhaseItemExtraIndex {
        self.extra_index.clone()
    }

    #[inline]
    fn batch_range_and_extra_index_mut(&mut self) -> (&mut Range<u32>, &mut PhaseItemExtraIndex) {
        (&mut self.batch_range, &mut self.extra_index)
    }
}

impl BinnedPhaseItem for AlphaMask3dChunkPrepass {
    type BatchSetKey = OpaqueChunkNoLightmap3dBatchSetKey;
    type BinKey = OpaqueNoLightmap3dBinKey;

    #[inline]
    fn new(
        batch_set_key: Self::BatchSetKey,
        bin_key: Self::BinKey,
        representative_entity: (Entity, MainEntity),
        batch_range: Range<u32>,
        extra_index: PhaseItemExtraIndex,
    ) -> Self {
        Self {
            batch_set_key,
            bin_key,
            representative_entity,
            batch_range,
            extra_index,
        }
    }
}

impl CachedRenderPipelinePhaseItem for AlphaMask3dChunkPrepass {
    #[inline]
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.batch_set_key.pipeline
    }
}

// Extract the render phases for the prepass

pub fn extract_camera_prepass_phase(
    mut opaque_3d_prepass_phases: ResMut<ViewBinnedChunksRenderPhases<Opaque3dChunkPrepass>>,
    mut alpha_mask_3d_prepass_phases: ResMut<ViewBinnedChunksRenderPhases<AlphaMask3dChunkPrepass>>,
    mut opaque_3d_deferred_phases: ResMut<ViewBinnedChunksRenderPhases<Opaque3dChunkDeferred>>,
    mut alpha_mask_3d_deferred_phases: ResMut<
        ViewBinnedChunksRenderPhases<AlphaMask3dChunkDeferred>,
    >,
    cameras_3d: Extract<
        Query<
            (
                Entity,
                &Camera,
                Has<NoIndirectDrawing>,
                Has<DepthPrepass>,
                Has<NormalPrepass>,
                Has<MotionVectorPrepass>,
                Has<DeferredPrepass>,
                // Has<DepthPrepassDoubleBuffer>,
                // Has<DeferredPrepassDoubleBuffer>,
            ),
            With<Camera3d>,
        >,
    >,
    mut live_entities: Local<HashSet<RetainedViewEntity>>,
    gpu_preprocessing_support: Res<GpuPreprocessingSupport>,
) {
    live_entities.clear();

    for (
        main_entity,
        camera,
        no_indirect_drawing,
        depth_prepass,
        normal_prepass,
        motion_vector_prepass,
        deferred_prepass,
        // depth_prepass_double_buffer,
        // deferred_prepass_double_buffer,
    ) in cameras_3d.iter()
    {
        if !camera.is_active {
            continue;
        }

        // If GPU culling is in use, use it (and indirect mode); otherwise, just
        // preprocess the meshes.
        let gpu_preprocessing_mode = gpu_preprocessing_support.min(if !no_indirect_drawing {
            GpuPreprocessingMode::Culling
        } else {
            GpuPreprocessingMode::PreprocessingOnly
        });

        // This is the main 3D camera, so we use the first subview index (0).
        let retained_view_entity = RetainedViewEntity::new(main_entity.into(), None, 0);

        if depth_prepass || normal_prepass || motion_vector_prepass {
            opaque_3d_prepass_phases.prepare_for_new_frame(
                retained_view_entity,
                gpu_preprocessing_mode,
                no_indirect_drawing,
            );
            alpha_mask_3d_prepass_phases.prepare_for_new_frame(
                retained_view_entity,
                gpu_preprocessing_mode,
                no_indirect_drawing,
            );
        } else {
            opaque_3d_prepass_phases.remove(&retained_view_entity);
            alpha_mask_3d_prepass_phases.remove(&retained_view_entity);
        }

        if deferred_prepass {
            opaque_3d_deferred_phases.prepare_for_new_frame(
                retained_view_entity,
                gpu_preprocessing_mode,
                no_indirect_drawing,
            );
            alpha_mask_3d_deferred_phases.prepare_for_new_frame(
                retained_view_entity,
                gpu_preprocessing_mode,
                no_indirect_drawing,
            );
        } else {
            opaque_3d_deferred_phases.remove(&retained_view_entity);
            alpha_mask_3d_deferred_phases.remove(&retained_view_entity);
        }
        live_entities.insert(retained_view_entity);
    }

    opaque_3d_prepass_phases.retain(|view_entity, _| live_entities.contains(view_entity));
    alpha_mask_3d_prepass_phases.retain(|view_entity, _| live_entities.contains(view_entity));
    opaque_3d_deferred_phases.retain(|view_entity, _| live_entities.contains(view_entity));
    alpha_mask_3d_deferred_phases.retain(|view_entity, _| live_entities.contains(view_entity));
}
