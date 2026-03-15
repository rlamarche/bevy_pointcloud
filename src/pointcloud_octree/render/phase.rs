use std::hash::Hash;
use std::ops::Range;

use crate::octree::storage::NodeId;
use crate::pointcloud_octree::asset::PointCloudOctree;
use crate::render::phase::PointCloud3dBatchSetKey;
use bevy_asset::AssetId;
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use bevy_platform::collections::HashMap;
use bevy_platform::collections::hash_map::Entry;
use bevy_render::render_phase::{
    CachedRenderPipelinePhaseItem, DrawError, DrawFunctionId, DrawFunctions, PhaseItem,
    PhaseItemBatchSetKey, PhaseItemExtraIndex, TrackedRenderPass,
};
use bevy_render::render_resource::CachedRenderPipelineId;
use bevy_render::sync_world::MainEntity;
use bevy_render::view::RetainedViewEntity;
use indexmap::IndexMap;

/// Data that must be identical in order to *batch* phase items together.
///
/// Note that a *batch set* (if multi-draw is in use) contains multiple batches.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PointCloudOctree3dBinKey {
    /// The asset that this phase item is associated with.
    ///
    /// Normally, this is the ID of the mesh, but for non-mesh items it might be
    /// the ID of another type of asset.
    pub asset_id: AssetId<PointCloudOctree>,
}

#[derive(Resource, Deref, DerefMut)]
pub struct ViewOctreeNodesRenderDepthPhases<BPI>(ViewOctreeNodesRenderPhases<BPI>)
where
    BPI: PointCloudOctreeBinnedPhaseItem;

impl<BPI> Default for ViewOctreeNodesRenderDepthPhases<BPI>
where
    BPI: PointCloudOctreeBinnedPhaseItem,
{
    fn default() -> Self {
        Self(Default::default())
    }
}

#[derive(Resource, Deref, DerefMut)]
pub struct ViewOctreeNodesRenderAttributePhases<BPI>(ViewOctreeNodesRenderPhases<BPI>)
where
    BPI: PointCloudOctreeBinnedPhaseItem;

impl<BPI> Default for ViewOctreeNodesRenderAttributePhases<BPI>
where
    BPI: PointCloudOctreeBinnedPhaseItem,
{
    fn default() -> Self {
        Self(Default::default())
    }
}

#[derive(Deref, DerefMut)]
pub struct ViewOctreeNodesRenderPhases<BPI>(
    pub HashMap<RetainedViewEntity, OctreeNodeRenderPhase<BPI>>,
)
where
    BPI: PointCloudOctreeBinnedPhaseItem;

impl<BPI> Default for ViewOctreeNodesRenderPhases<BPI>
where
    BPI: PointCloudOctreeBinnedPhaseItem,
{
    fn default() -> Self {
        Self(HashMap::default())
    }
}

impl<BPI> ViewOctreeNodesRenderPhases<BPI>
where
    BPI: PointCloudOctreeBinnedPhaseItem,
{
    pub fn prepare_for_new_frame(&mut self, retained_view_entity: RetainedViewEntity) {
        match self.entry(retained_view_entity) {
            Entry::Occupied(mut entry) => entry.get_mut().prepare_for_new_frame(),
            Entry::Vacant(entry) => {
                entry.insert(OctreeNodeRenderPhase::<BPI>::new());
            }
        }
    }
}

pub struct OctreeNodeRenderPhase<BPI>
where
    BPI: PointCloudOctreeBinnedPhaseItem,
{
    pub phases: IndexMap<(BPI::BatchSetKey, BPI::BinKey), Vec<BPI>>,
}

impl<BPI> OctreeNodeRenderPhase<BPI>
where
    BPI: PointCloudOctreeBinnedPhaseItem,
{
    fn new() -> Self {
        Self {
            phases: IndexMap::default(),
        }
    }

    pub fn prepare_for_new_frame(&mut self) {
        self.phases.clear();
    }

    /// Bins a new entity.
    ///
    /// The `phase_type` parameter specifies whether the entity is a
    /// preprocessable mesh and whether it can be binned with meshes of the same
    /// type.
    pub fn add(
        &mut self,
        batch_set_key: BPI::BatchSetKey,
        bin_key: BPI::BinKey,
        (entity, main_entity): (Entity, MainEntity),
        // input_uniform_index: InputUniformIndex,
        node_ids: Vec<NodeId>,
    ) {
        let phase_item = BPI::new(
            batch_set_key.clone(),
            bin_key.clone(),
            (entity, main_entity),
            0..1,
            PhaseItemExtraIndex::None,
            node_ids,
        );

        match self.phases.entry((batch_set_key, bin_key).clone()) {
            indexmap::map::Entry::Occupied(mut entry) => {
                entry.get_mut().push(phase_item);
            }
            indexmap::map::Entry::Vacant(entry) => {
                let mut phase_items = Vec::new();
                phase_items.push(phase_item);
                entry.insert(phase_items);
            }
        }
    }

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

        {
            let draw_functions = world.resource::<DrawFunctions<BPI>>();
            let mut draw_functions = draw_functions.write();

            for ((_batch_set_key, _bin_key), phase_items) in &self.phases {
                let phase_item = &phase_items[0];

                // get common draw function
                let Some(draw_function) = draw_functions.get_mut(phase_item.draw_function()) else {
                    continue;
                };

                for phase_item in phase_items {
                    draw_function.draw(world, render_pass, view, &phase_item)?;
                }
            }
        }

        Ok(())
    }
}

/// Represents phase items that are placed into bins. The `BinKey` specifies
/// which bin they're to be placed in. Bin keys are sorted, and items within the
/// same bin are eligible to be batched together. The elements within the bins
/// aren't themselves sorted.
///
/// An example of a binned phase item is `Opaque3d`, for which the rendering
/// order isn't critical.
pub trait PointCloudOctreeBinnedPhaseItem: CachedRenderPipelinePhaseItem + 'static {
    /// The key used for binning [`PhaseItem`]s into bins. Order the members of
    /// [`BinnedPhaseItem::BinKey`] by the order of binding for best
    /// performance. For example, pipeline id, draw function id, mesh asset id,
    /// lowest variable bind group id such as the material bind group id, and
    /// its dynamic offsets if any, next bind group and offsets, etc. This
    /// reduces the need for rebinding between bins and improves performance.
    type BinKey: Clone + Send + Sync + PartialEq + Eq + Ord + Hash;

    /// The key used to combine batches into batch sets.
    ///
    /// A *batch set* is a set of meshes that can potentially be multi-drawn
    /// together.
    type BatchSetKey: PhaseItemBatchSetKey;

    /// Creates a new binned phase item from the key and per-entity data.
    ///
    /// Unlike [`SortedPhaseItem`]s, this is generally called "just in time"
    /// before rendering. The resulting phase item isn't stored in any data
    /// structures, resulting in significant memory savings.
    fn new(
        batch_set_key: Self::BatchSetKey,
        bin_key: Self::BinKey,
        representative_entity: (Entity, MainEntity),
        batch_range: Range<u32>,
        extra_index: PhaseItemExtraIndex,
        node_ids: Vec<NodeId>,
    ) -> Self;

    fn node_ids(&self) -> &[NodeId];
}

pub struct PointCloudOctree3dNodePhase {
    /// Determines which objects can be placed into a *batch set*.
    ///
    /// Objects in a single batch set can potentially be multi-drawn together,
    /// if it's enabled and the current platform supports it.
    pub batch_set_key: PointCloud3dBatchSetKey,
    /// The key, which determines which can be batched.
    pub bin_key: PointCloudOctree3dBinKey,
    /// An entity from which data will be fetched, including the mesh if
    /// applicable.
    pub representative_entity: (Entity, MainEntity),
    /// The ranges of instances.
    pub batch_range: Range<u32>,
    /// An extra index, which is either a dynamic offset or an index in the
    /// indirect parameters list.
    pub extra_index: PhaseItemExtraIndex,
    /// The node id in the octree
    pub node_ids: Vec<NodeId>,
}

impl PhaseItem for PointCloudOctree3dNodePhase {
    #[inline]
    fn entity(&self) -> Entity {
        self.representative_entity.0
    }

    #[inline]
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

    fn extra_index(&self) -> PhaseItemExtraIndex {
        self.extra_index.clone()
    }

    fn batch_range_and_extra_index_mut(&mut self) -> (&mut Range<u32>, &mut PhaseItemExtraIndex) {
        (&mut self.batch_range, &mut self.extra_index)
    }
}

impl PointCloudOctreeBinnedPhaseItem for PointCloudOctree3dNodePhase {
    type BinKey = PointCloudOctree3dBinKey;
    type BatchSetKey = PointCloud3dBatchSetKey;

    #[inline]
    fn new(
        batch_set_key: Self::BatchSetKey,
        bin_key: Self::BinKey,
        representative_entity: (Entity, MainEntity),
        batch_range: Range<u32>,
        extra_index: PhaseItemExtraIndex,
        node_ids: Vec<NodeId>,
    ) -> Self {
        PointCloudOctree3dNodePhase {
            batch_set_key,
            bin_key,
            representative_entity,
            batch_range,
            extra_index,
            node_ids,
        }
    }

    fn node_ids(&self) -> &[NodeId] {
        &self.node_ids
    }
}

impl CachedRenderPipelinePhaseItem for PointCloudOctree3dNodePhase {
    #[inline]
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.batch_set_key.pipeline
    }
}
