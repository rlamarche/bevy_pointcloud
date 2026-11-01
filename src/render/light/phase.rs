use std::ops::Range;

use bevy::{
    camera::{visibility::RenderLayers, Camera3d},
    ecs::{
        entity::Entity,
        query::{Has, With},
        system::{Local, Query, Res, ResMut},
    },
    material::{descriptor::CachedRenderPipelineId, labels::DrawFunctionId},
    pbr::{
        ExtractedDirectionalLight, ShadowBinKey, MAX_CASCADES_PER_LIGHT, MAX_DIRECTIONAL_LIGHTS,
    },
    platform::collections::HashSet,
    render::{
        batching::gpu_preprocessing::{GpuPreprocessingMode, GpuPreprocessingSupport},
        camera::SortedCameras,
        mesh::allocator::MeshSlabs,
        render_phase::{
            BinnedPhaseItem, CachedRenderPipelinePhaseItem, PhaseItem, PhaseItemBatchSetKey,
            PhaseItemExtraIndex,
        },
        sync_world::MainEntity,
        view::{NoIndirectDrawing, RetainedViewEntity},
    },
};

use crate::render::render_phase::ViewBinnedChunksRenderPhases;

pub struct ShadowChunk {
    /// Determines which objects can be placed into a *batch set*.
    ///
    /// Objects in a single batch set can potentially be multi-drawn together,
    /// if it's enabled and the current platform supports it.
    pub batch_set_key: ShadowChunkBatchSetKey,
    /// Information that separates items into bins.
    pub bin_key: ShadowBinKey,
    pub representative_entity: (Entity, MainEntity),
    pub batch_range: Range<u32>,
    pub extra_index: PhaseItemExtraIndex,
}

/// Information that must be identical in order to place opaque meshes in the
/// same *batch set*.
///
/// A batch set is a set of batches that can be multi-drawn together, if
/// multi-draw is in use.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShadowChunkBatchSetKey {
    /// The identifier of the render pipeline.
    pub pipeline: CachedRenderPipelineId,

    /// The function used to draw.
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

impl PhaseItemBatchSetKey for ShadowChunkBatchSetKey {
    fn indexed(&self) -> bool {
        self.splat_slabs.index_slab_id.is_some()
    }
}

impl PhaseItem for ShadowChunk {
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

impl BinnedPhaseItem for ShadowChunk {
    type BatchSetKey = ShadowChunkBatchSetKey;
    type BinKey = ShadowBinKey;

    #[inline]
    fn new(
        batch_set_key: Self::BatchSetKey,
        bin_key: Self::BinKey,
        representative_entity: (Entity, MainEntity),
        batch_range: Range<u32>,
        extra_index: PhaseItemExtraIndex,
    ) -> Self {
        ShadowChunk {
            batch_set_key,
            bin_key,
            representative_entity,
            batch_range,
            extra_index,
        }
    }
}

impl CachedRenderPipelinePhaseItem for ShadowChunk {
    #[inline]
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.batch_set_key.pipeline
    }
}

pub fn prepare_light_phase(
    views: Query<
        (
            Entity,
            MainEntity,
            Option<&RenderLayers>,
            Has<NoIndirectDrawing>,
        ),
        With<Camera3d>,
    >,
    mut shadow_render_phases: ResMut<ViewBinnedChunksRenderPhases<ShadowChunk>>,
    mut live_shadow_mapping_lights: Local<HashSet<RetainedViewEntity>>,
    directional_lights: Query<(
        Entity,
        &MainEntity,
        &ExtractedDirectionalLight,
        &RenderLayers,
    )>,
    sorted_cameras: Res<SortedCameras>,
    gpu_preprocessing_support: Res<GpuPreprocessingSupport>,
) {
    live_shadow_mapping_lights.clear();

    let directional_light_entities: Vec<_> = directional_lights
        .iter()
        .map(|(entity, _, _, _)| entity)
        .collect::<Vec<_>>();

    // set up light data for each view
    for (entity, camera_main_entity, maybe_layers, no_indirect_drawing) in sorted_cameras
        .0
        .iter()
        .filter_map(|sorted_camera| views.get(sorted_camera.entity).ok())
    {
        let view_layers = maybe_layers.unwrap_or_default();

        let gpu_preprocessing_mode = gpu_preprocessing_support.min(if !no_indirect_drawing {
            GpuPreprocessingMode::Culling
        } else {
            GpuPreprocessingMode::PreprocessingOnly
        });

        for (_light_index, light_entity) in directional_light_entities
            .iter()
            .filter(|light_entity| {
                directional_lights
                    .get(**light_entity)
                    .unwrap()
                    .3
                    .intersects(view_layers)
            })
            .enumerate()
            .take(MAX_DIRECTIONAL_LIGHTS)
        {
            let (_, light_main_entity, light, _) = directional_lights.get(*light_entity).unwrap();

            let Some(cascades) = light.cascades.get(&entity) else {
                continue;
            };

            let cascades = cascades.iter().take(MAX_CASCADES_PER_LIGHT);

            for (cascade_index, _) in cascades.enumerate() {
                let retained_view_entity = RetainedViewEntity::new(
                    *light_main_entity,
                    Some(camera_main_entity.into()),
                    cascade_index as u32,
                );

                // Subsequent views with the same light entity will **NOT** reuse the same shadow
                // map (Because the cascades are unique to each view)
                // TODO: Implement GPU culling for shadow passes.
                shadow_render_phases.prepare_for_new_frame(
                    retained_view_entity,
                    gpu_preprocessing_mode,
                    no_indirect_drawing,
                );
                live_shadow_mapping_lights.insert(retained_view_entity);
            }
        }
    }

    shadow_render_phases.retain(|view_entity, _| live_shadow_mapping_lights.contains(view_entity));
}
