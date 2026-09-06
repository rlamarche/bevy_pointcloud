use std::ops::Range;

use bevy::{
    camera::{Camera, Camera3d},
    core_pipeline::core_3d::{Opaque3dBatchSetKey, Opaque3dBinKey},
    ecs::{
        entity::Entity,
        query::{Has, With},
        system::{Commands, Local, Query, Res, ResMut},
    },
    material::{descriptor::CachedRenderPipelineId, labels::DrawFunctionId},
    platform::collections::HashSet,
    render::{
        batching::gpu_preprocessing::{GpuPreprocessingMode, GpuPreprocessingSupport},
        render_phase::{
            BinnedPhaseItem, CachedRenderPipelinePhaseItem, PhaseItem, PhaseItemExtraIndex,
            ViewBinnedRenderPhases,
        },
        sync_world::{MainEntity, RenderEntity},
        view::{NoIndirectDrawing, RetainedViewEntity},
        Extract,
    },
};

/// Opaque phase of the 3D multipass.
///
/// Sorted by pipeline, then by mesh to improve batching.
///
/// Used to render all 3D meshes with materials that have no transparency.
pub struct Opaque3dMultipass {
    /// Determines which objects can be placed into a *batch set*.
    ///
    /// Objects in a single batch set can potentially be multi-drawn together,
    /// if it's enabled and the current platform supports it.
    pub batch_set_key: Opaque3dBatchSetKey,
    /// Information that separates items into bins.
    pub bin_key: Opaque3dBinKey,

    /// An entity from which Bevy fetches data common to all instances in this
    /// batch, such as the mesh.
    pub representative_entity: (Entity, MainEntity),
    pub batch_range: Range<u32>,
    pub extra_index: PhaseItemExtraIndex,
}

impl CachedRenderPipelinePhaseItem for Opaque3dMultipass {
    #[inline]
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.batch_set_key.pipeline
    }
}

impl PhaseItem for Opaque3dMultipass {
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

impl BinnedPhaseItem for Opaque3dMultipass {
    type BatchSetKey = Opaque3dBatchSetKey;
    type BinKey = Opaque3dBinKey;

    #[inline]
    fn new(
        batch_set_key: Self::BatchSetKey,
        bin_key: Self::BinKey,
        representative_entity: (Entity, MainEntity),
        batch_range: Range<u32>,
        extra_index: PhaseItemExtraIndex,
    ) -> Self {
        Opaque3dMultipass {
            batch_set_key,
            bin_key,
            representative_entity,
            batch_range,
            extra_index,
        }
    }
}

pub fn extract_camera_multipass_phase(
    mut commands: Commands,
    mut opaque_3d_multipass_phases: ResMut<ViewBinnedRenderPhases<Opaque3dMultipass>>,
    cameras_3d: Extract<
        Query<(Entity, RenderEntity, &Camera, Has<NoIndirectDrawing>), With<Camera3d>>,
    >,
    mut live_entities: Local<HashSet<RetainedViewEntity>>,
    gpu_preprocessing_support: Res<GpuPreprocessingSupport>,
) {
    live_entities.clear();

    for (main_entity, entity, camera, no_indirect_drawing) in cameras_3d.iter() {
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

        opaque_3d_multipass_phases
            .prepare_for_new_frame(retained_view_entity, gpu_preprocessing_mode);

        live_entities.insert(retained_view_entity);
    }

    opaque_3d_multipass_phases.retain(|view_entity, _| live_entities.contains(view_entity));
}
