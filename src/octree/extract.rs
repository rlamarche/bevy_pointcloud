use crate::octree::asset::{NodeId, Octree, OctreeNode};
use crate::octree::render_asset::RenderOctree;
use bevy_app::{App, Plugin, SubApp};
use bevy_asset::{Asset, AssetEvent, AssetId, Assets};
use bevy_camera::primitives::Aabb;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleConfigs;
use bevy_ecs::system::{
    ScheduleSystem, StaticSystemParam, SystemParam, SystemParamItem, SystemState,
};
use bevy_log::prelude::*;
use bevy_platform::collections::{HashMap, HashSet};
use bevy_reflect::{Reflect, TypePath};
use bevy_render::render_resource::AsBindGroupError;
use bevy_render::{Extract, ExtractSchedule, MainWorld, Render, RenderApp, RenderSystems};
use core::sync::atomic::{AtomicUsize, Ordering};
use std::fmt::Debug;
use std::marker::PhantomData;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PrepareOctreeNodeError<E: Send + Sync + 'static> {
    #[error("Failed to prepare asset")]
    RetryNextUpdate(E),
    #[error("Failed to build bind group: {0}")]
    AsBindGroupError(AsBindGroupError),
}

/// The system set during which we extract modified octree to the render world.
#[derive(SystemSet, Clone, PartialEq, Eq, Debug, Hash)]
pub struct OctreeExtractionSystems;

/// Describes how an octree node gets extracted and prepared for rendering.
///
/// In the [`ExtractSchedule`] step the [`RenderOctreeNode::SourceOctreeNode`] is transferred
/// from the "main world" into the "render world".
///
/// After that in the [`RenderSystems::PrepareAssets`] step the extracted octree nodes
/// are transformed into their GPU-representation of type [`RenderOctreeNode`].
pub trait RenderOctreeNode: Send + Sync + Sized + 'static + Clone + Debug + TypePath {
    /// The representation of the octree node in the "main world".
    type SourceOctreeNode: Send + Sync + Clone + Debug + Default + TypePath;

    /// Specifies all ECS data required by [`RenderAsset::prepare_asset`].
    ///
    /// For convenience use the [`lifetimeless`](bevy_ecs::system::lifetimeless) [`SystemParam`].
    type Param: SystemParam;

    /// Size of the data the asset will upload to the gpu. Specifying a return value
    /// will allow the asset to be throttled via [`RenderAssetBytesPerFrame`].
    #[inline]
    #[expect(
        unused_variables,
        reason = "The parameters here are intentionally unused by the default implementation; however, putting underscores here will result in the underscores being copied by rust-analyzer's tab completion."
    )]
    fn byte_len(source_node: &Self::SourceOctreeNode) -> Option<usize> {
        None
    }

    /// Prepares the [`RenderAsset::SourceAsset`] for the GPU by transforming it into a [`RenderAsset`].
    ///
    /// ECS data may be accessed via `param`.
    fn prepare_octree_node(
        source_node: Self::SourceOctreeNode,
        asset_id: AssetId<Octree<Self::SourceOctreeNode>>,
        node_id: NodeId,
        param: &mut SystemParamItem<Self::Param>,
    ) -> Result<Self, PrepareOctreeNodeError<Self::SourceOctreeNode>>;

    /// Called whenever the [`RenderAsset::SourceAsset`] has been removed.
    ///
    /// You can implement this method if you need to access ECS data (via
    /// `_param`) in order to perform cleanup tasks when the asset is removed.
    ///
    /// The default implementation does nothing.
    fn unload_asset(
        _source_asset: AssetId<Octree<Self::SourceOctreeNode>>,
        _param: &mut SystemParamItem<Self::Param>,
    ) {
    }
}

/// This plugin extracts the changed assets from the "app world" into the "render world"
/// and prepares them for the GPU. They can then be accessed from the [`RenderOctreeNode`] resource.
///
/// Therefore it sets up the [`ExtractSchedule`] and
/// [`RenderSystems::PrepareAssets`] steps for the specified [`RenderAsset`].
///
/// The `AFTER` generic parameter can be used to specify that `A::prepare_asset` should not be run until
/// `prepare_assets::<AFTER>` has completed. This allows the `prepare_asset` function to depend on another
/// prepared [`RenderAsset`], for example `Mesh::prepare_asset` relies on `RenderAssets::<GpuImage>` for morph
/// targets, so the plugin is created as `RenderAssetPlugin::<RenderMesh, GpuImage>::default()`.
pub struct RenderOctreePlugin<A: RenderOctreeNode, AFTER: RenderOctreeDependency + 'static = ()> {
    phantom: PhantomData<fn() -> (A, AFTER)>,
}

impl<A: RenderOctreeNode, AFTER: RenderOctreeDependency + 'static> Default
    for RenderOctreePlugin<A, AFTER>
{
    fn default() -> Self {
        Self {
            phantom: Default::default(),
        }
    }
}

impl<A: RenderOctreeNode, AFTER: RenderOctreeDependency + 'static> Plugin
    for RenderOctreePlugin<A, AFTER>
{
    fn build(&self, app: &mut App) {
        app.init_resource::<CachedExtractRenderAssetSystemState<A>>();
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<ExtractedOctrees<A>>()
                .init_resource::<RenderOctrees<A>>()
                .init_resource::<PrepareNextFrameOctreeNodes<A>>()
                .add_systems(
                    ExtractSchedule,
                    extract_render_octree_node::<A>.in_set(OctreeExtractionSystems),
                );
            AFTER::register_system(
                render_app,
                prepare_assets::<A>.in_set(RenderSystems::PrepareAssets),
            );
        }
    }
}

// helper to allow specifying dependencies between render assets
pub trait RenderOctreeDependency {
    fn register_system(render_app: &mut SubApp, system: ScheduleConfigs<ScheduleSystem>);
}

impl RenderOctreeDependency for () {
    fn register_system(render_app: &mut SubApp, system: ScheduleConfigs<ScheduleSystem>) {
        render_app.add_systems(Render, system);
    }
}

// impl<A: RenderOctreeNode> RenderOctreeDependency for A {
//     fn register_system(render_app: &mut SubApp, system: ScheduleConfigs<ScheduleSystem>) {
//         render_app.add_systems(Render, system.after(prepare_assets::<A>));
//     }
// }

#[derive(Resource)]
pub struct ExtractedOctrees<A: RenderOctreeNode> {
    /// The assets extracted this frame.
    ///
    /// These are assets that were either added or modified this frame.
    pub extracted_octrees: Vec<(
        AssetId<Octree<A::SourceOctreeNode>>,
        Vec<OctreeNode<A::SourceOctreeNode>>,
    )>,

    /// IDs of the assets that were removed this frame.
    ///
    /// These assets will not be present in [`ExtractedAssets::extracted`].
    pub removed_assets: HashSet<AssetId<Octree<A::SourceOctreeNode>>>,
    pub removed_nodes: HashMap<AssetId<Octree<A::SourceOctreeNode>>, Vec<NodeId>>,

    /// IDs of the assets that were modified this frame.
    pub modified_assets: HashSet<AssetId<Octree<A::SourceOctreeNode>>>,
    pub modified_nodes: HashMap<AssetId<Octree<A::SourceOctreeNode>>, Vec<NodeId>>,

    /// IDs of the assets that were added this frame.
    pub added_assets: HashSet<AssetId<Octree<A::SourceOctreeNode>>>,
    pub added_nodes: HashMap<AssetId<Octree<A::SourceOctreeNode>>, Vec<NodeId>>,
}

impl<A: RenderOctreeNode> Default for ExtractedOctrees<A> {
    fn default() -> Self {
        Self {
            extracted_octrees: Default::default(),
            removed_assets: Default::default(),
            removed_nodes: Default::default(),
            modified_assets: Default::default(),
            modified_nodes: Default::default(),
            added_assets: Default::default(),
            added_nodes: Default::default(),
        }
    }
}

/// Stores all GPU representations ([`RenderAsset`])
/// of [`RenderAsset::SourceAsset`] as long as they exist.
#[derive(Resource, Reflect)]
pub struct RenderOctrees<A: RenderOctreeNode>(
    HashMap<AssetId<Octree<A::SourceOctreeNode>>, RenderOctree<A>>,
);

impl<A: RenderOctreeNode> Default for RenderOctrees<A> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<A: RenderOctreeNode> RenderOctrees<A> {
    pub fn get(
        &self,
        id: impl Into<AssetId<Octree<A::SourceOctreeNode>>>,
    ) -> Option<&RenderOctree<A>> {
        self.0.get(&id.into())
    }

    pub fn get_mut(
        &mut self,
        id: impl Into<AssetId<Octree<A::SourceOctreeNode>>>,
    ) -> Option<&mut RenderOctree<A>> {
        self.0.get_mut(&id.into())
    }

    pub fn insert(
        &mut self,
        id: impl Into<AssetId<Octree<A::SourceOctreeNode>>>,
        value: RenderOctree<A>,
    ) -> Option<RenderOctree<A>> {
        self.0.insert(id.into(), value)
    }

    pub fn remove(
        &mut self,
        id: impl Into<AssetId<Octree<A::SourceOctreeNode>>>,
    ) -> Option<RenderOctree<A>> {
        self.0.remove(&id.into())
    }

    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (AssetId<Octree<A::SourceOctreeNode>>, &RenderOctree<A>)> {
        self.0.iter().map(|(k, v)| (*k, v))
    }

    pub fn iter_mut(
        &mut self,
    ) -> impl Iterator<Item = (AssetId<Octree<A::SourceOctreeNode>>, &mut RenderOctree<A>)> {
        self.0.iter_mut().map(|(k, v)| (*k, v))
    }
}

#[derive(Resource)]
struct CachedExtractRenderAssetSystemState<A: RenderOctreeNode> {
    state: SystemState<(
        MessageReader<'static, 'static, AssetEvent<Octree<A::SourceOctreeNode>>>,
        ResMut<'static, Assets<Octree<A::SourceOctreeNode>>>,
    )>,
}

impl<A: RenderOctreeNode> FromWorld for CachedExtractRenderAssetSystemState<A> {
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        Self {
            state: SystemState::new(world),
        }
    }
}

/// This system extracts all created or modified assets of the corresponding [`RenderAsset::SourceAsset`] type
/// into the "render world".
pub(crate) fn extract_render_octree_node<A: RenderOctreeNode>(
    mut commands: Commands,
    mut main_world: ResMut<MainWorld>,
) {
    main_world.resource_scope(
        |world, mut cached_state: Mut<CachedExtractRenderAssetSystemState<A>>| {
            let (mut events, mut assets) = cached_state.state.get_mut(world);

            let mut needs_extracting = <HashSet<_>>::default();
            let mut removed_assets = <HashSet<_>>::default();
            let mut modified_assets = <HashSet<_>>::default();

            for event in events.read() {
                match event {
                    AssetEvent::Added { id } => {
                        info!("Octree asset {} added", id);
                        needs_extracting.insert(*id);
                    }
                    AssetEvent::Modified { id } => {
                        needs_extracting.insert(*id);
                        modified_assets.insert(*id);
                    }
                    AssetEvent::Removed { .. } => {
                        // We don't care that the asset was removed from Assets<T> in the main world.
                        // An asset is only removed from RenderAssets<T> when its last handle is dropped (AssetEvent::Unused).
                    }
                    AssetEvent::Unused { id } => {
                        needs_extracting.remove(id);
                        modified_assets.remove(id);
                        removed_assets.insert(*id);
                    }
                    AssetEvent::LoadedWithDependencies { .. } => {
                        // nothing to do
                    }
                }
            }

            let mut extracted_octree_nodes = Vec::new();
            let mut added_assets = <HashSet<_>>::default();

            let mut removed_nodes = <HashMap<_, _>>::default();
            let mut modified_nodes = <HashMap<_, _>>::default();
            let mut added_nodes = <HashMap<_, _>>::default();
            for id in needs_extracting.drain() {
                if let Some(asset) = assets.get_mut(id) {
                    let mut node_ids = HashSet::new();
                    asset.added.iter().for_each(|node_id| { node_ids.insert(*node_id); });
                    asset.modified.iter().for_each(|node_id| { node_ids.insert(*node_id); });

                    // extract and clone new or modified nodes
                    let extracted_nodes = node_ids.drain().flat_map(|node_id| asset.nodes.get(node_id)).map(|node| node.clone()).collect::<Vec<_>>();

                    extracted_octree_nodes.push((id, extracted_nodes));

                    added_nodes.insert(id, asset.added.clone());
                    modified_nodes.insert(id, asset.modified.clone());
                    removed_nodes.insert(id, asset.removed.clone());

                    added_assets.insert(id);

                    asset.modified.clear();
                    asset.added.clear();
                    asset.removed.clear();
                }
            }

            commands.insert_resource(ExtractedOctrees::<A> {
                extracted_octrees: extracted_octree_nodes,
                removed_assets,
                removed_nodes,
                modified_assets,
                modified_nodes,
                added_assets,
                added_nodes,
            });
            cached_state.state.apply(world);
        },
    );
}

// TODO: consider storing inside system?
/// All assets that should be prepared next frame.
#[derive(Resource)]
pub struct PrepareNextFrameOctreeNodes<A: RenderOctreeNode> {
    assets: Vec<(
        AssetId<Octree<A::SourceOctreeNode>>,
        OctreeNode<A::SourceOctreeNode>,
    )>,
}

impl<A: RenderOctreeNode> Default for PrepareNextFrameOctreeNodes<A> {
    fn default() -> Self {
        Self {
            assets: Default::default(),
        }
    }
}

/// This system prepares all assets of the corresponding [`RenderAsset::SourceAsset`] type
/// which where extracted this frame for the GPU.
pub fn prepare_assets<A: RenderOctreeNode>(
    mut extracted_assets: ResMut<ExtractedOctrees<A>>,
    mut render_assets: ResMut<RenderOctrees<A>>,
    mut prepare_next_frame: ResMut<PrepareNextFrameOctreeNodes<A>>,
    param: StaticSystemParam<<A as RenderOctreeNode>::Param>,
    bpf: Res<RenderAssetBytesPerFrameLimiter>,
) {
    let mut wrote_asset_count = 0;

    let mut param = param.into_inner();
    let queued_assets = core::mem::take(&mut prepare_next_frame.assets);
    for (id, extracted_octree_node) in queued_assets {
        if extracted_assets
            .removed_nodes
            .get(&id)
            .map(|nodes| nodes.contains(&extracted_octree_node.id))
            .unwrap_or(false)
            || extracted_assets
                .added_nodes
                .get(&id)
                .map(|nodes| nodes.contains(&extracted_octree_node.id))
                .unwrap_or(false)
        {
            // skip previous frame's assets that have been removed or updated
            continue;
        }

        let write_bytes = if let Some(size) = A::byte_len(&extracted_octree_node.data) {
            // we could check if available bytes > byte_len here, but we want to make some
            // forward progress even if the asset is larger than the max bytes per frame.
            // this way we always write at least one (sized) asset per frame.
            // in future we could also consider partial asset uploads.
            if bpf.exhausted() {
                prepare_next_frame.assets.push((id, extracted_octree_node));
                continue;
            }
            size
        } else {
            0
        };

        let mut render_asset = match render_assets.get_mut(id) {
            None => {
                let render_octree = RenderOctree::<A>::default();
                render_assets.insert(id, render_octree);
                render_assets.get_mut(id).unwrap()
            }
            Some(asset) => asset,
        };

        match A::prepare_octree_node(
            extracted_octree_node.data,
            id,
            extracted_octree_node.id,
            &mut param,
        ) {
            Ok(prepared_octree_node) => {
                let render_octree_node = OctreeNode::<A> {
                    id: extracted_octree_node.id,
                    parent_id: extracted_octree_node.parent_id.clone(),
                    children: extracted_octree_node.children.clone(),
                    children_mask: extracted_octree_node.children_mask.clone(),
                    bounding_box: extracted_octree_node.bounding_box.clone(),
                    data: prepared_octree_node,
                };
                render_asset.insert(extracted_octree_node.id, render_octree_node);

                bpf.write_bytes(write_bytes);
                wrote_asset_count += 1;
            }
            Err(PrepareOctreeNodeError::RetryNextUpdate(extracted_data)) => {
                prepare_next_frame.assets.push((
                    id,
                    OctreeNode::<A::SourceOctreeNode> {
                        id: extracted_octree_node.id,
                        parent_id: extracted_octree_node.parent_id,
                        children: extracted_octree_node.children,
                        children_mask: extracted_octree_node.children_mask,
                        bounding_box: extracted_octree_node.bounding_box,
                        data: extracted_data,
                    },
                ));
            }
            Err(PrepareOctreeNodeError::AsBindGroupError(e)) => {
                error!(
                    "{} Bind group construction failed: {e}",
                    core::any::type_name::<A>()
                );
            }
        }
    }

    for removed in extracted_assets.removed_assets.drain() {
        render_assets.remove(removed);
        A::unload_asset(removed, &mut param);
    }

    for (id, extracted_octree_nodes) in extracted_assets.extracted_octrees.drain(..) {
        let mut render_asset = match render_assets.get_mut(id) {
            None => {
                let render_octree = RenderOctree::<A>::default();
                render_assets.insert(id, render_octree);
                render_assets.get_mut(id).unwrap()
            }
            Some(asset) => asset,
        };

        for extracted_octree_node in extracted_octree_nodes {
            let write_bytes = if let Some(size) = A::byte_len(&extracted_octree_node.data) {
                if bpf.exhausted() {
                    prepare_next_frame.assets.push((id, extracted_octree_node));
                    continue;
                }
                size
            } else {
                0
            };

            match A::prepare_octree_node(
                extracted_octree_node.data,
                id,
                extracted_octree_node.id,
                &mut param,
            ) {
                Ok(prepared_octree_node) => {
                    let render_octree_node = OctreeNode::<A> {
                        id: extracted_octree_node.id,
                        parent_id: extracted_octree_node.parent_id.clone(),
                        children: extracted_octree_node.children.clone(),
                        children_mask: extracted_octree_node.children_mask.clone(),
                        bounding_box: extracted_octree_node.bounding_box.clone(),
                        data: prepared_octree_node,
                    };
                    render_asset.insert(extracted_octree_node.id, render_octree_node);

                    bpf.write_bytes(write_bytes);
                    wrote_asset_count += 1;
                }
                Err(PrepareOctreeNodeError::RetryNextUpdate(extracted_data)) => {
                    prepare_next_frame.assets.push((
                        id,
                        OctreeNode::<A::SourceOctreeNode> {
                            id: extracted_octree_node.id,
                            parent_id: extracted_octree_node.parent_id,
                            children: extracted_octree_node.children,
                            children_mask: extracted_octree_node.children_mask,
                            bounding_box: extracted_octree_node.bounding_box,
                            data: extracted_data,
                        },
                    ));
                }
                Err(PrepareOctreeNodeError::AsBindGroupError(e)) => {
                    error!(
                        "{} Bind group construction failed: {e}",
                        core::any::type_name::<A>()
                    );
                }
            }
        }
    }

    if bpf.exhausted() && !prepare_next_frame.assets.is_empty() {
        debug!(
            "{} write budget exhausted with {} assets remaining (wrote {})",
            core::any::type_name::<A>(),
            prepare_next_frame.assets.len(),
            wrote_asset_count
        );
    }
}

pub fn reset_render_asset_bytes_per_frame(
    mut bpf_limiter: ResMut<RenderAssetBytesPerFrameLimiter>,
) {
    bpf_limiter.reset();
}

pub fn extract_render_asset_bytes_per_frame(
    bpf: Extract<Res<RenderAssetBytesPerFrame>>,
    mut bpf_limiter: ResMut<RenderAssetBytesPerFrameLimiter>,
) {
    bpf_limiter.max_bytes = bpf.max_bytes;
}

/// A resource that defines the amount of data allowed to be transferred from CPU to GPU
/// each frame, preventing choppy frames at the cost of waiting longer for GPU assets
/// to become available.
#[derive(Resource, Default)]
pub struct RenderAssetBytesPerFrame {
    pub max_bytes: Option<usize>,
}

impl RenderAssetBytesPerFrame {
    /// `max_bytes`: the number of bytes to write per frame.
    ///
    /// This is a soft limit: only full assets are written currently, uploading stops
    /// after the first asset that exceeds the limit.
    ///
    /// To participate, assets should implement [`RenderAsset::byte_len`]. If the default
    /// is not overridden, the assets are assumed to be small enough to upload without restriction.
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes: Some(max_bytes),
        }
    }
}

/// A render-world resource that facilitates limiting the data transferred from CPU to GPU
/// each frame, preventing choppy frames at the cost of waiting longer for GPU assets
/// to become available.
#[derive(Resource, Default)]
pub struct RenderAssetBytesPerFrameLimiter {
    /// Populated by [`RenderAssetBytesPerFrame`] during extraction.
    pub max_bytes: Option<usize>,
    /// Bytes written this frame.
    pub bytes_written: AtomicUsize,
}

impl RenderAssetBytesPerFrameLimiter {
    /// Reset the available bytes. Called once per frame during extraction by [`crate::RenderPlugin`].
    pub fn reset(&mut self) {
        if self.max_bytes.is_none() {
            return;
        }
        self.bytes_written.store(0, Ordering::Relaxed);
    }

    /// Check how many bytes are available for writing.
    pub fn available_bytes(&self, required_bytes: usize) -> usize {
        if let Some(max_bytes) = self.max_bytes {
            let total_bytes = self
                .bytes_written
                .fetch_add(required_bytes, Ordering::Relaxed);

            // The bytes available is the inverse of the amount we overshot max_bytes
            if total_bytes >= max_bytes {
                required_bytes.saturating_sub(total_bytes - max_bytes)
            } else {
                required_bytes
            }
        } else {
            required_bytes
        }
    }

    /// Decreases the available bytes for the current frame.
    pub(crate) fn write_bytes(&self, bytes: usize) {
        if self.max_bytes.is_some() && bytes > 0 {
            self.bytes_written.fetch_add(bytes, Ordering::Relaxed);
        }
    }

    /// Returns `true` if there are no remaining bytes available for writing this frame.
    pub(crate) fn exhausted(&self) -> bool {
        if let Some(max_bytes) = self.max_bytes {
            let bytes_written = self.bytes_written.load(Ordering::Relaxed);
            bytes_written >= max_bytes
        } else {
            false
        }
    }
}
