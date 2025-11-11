use crate::octree::asset::{NodeId, Octree, OctreeNode};
use crate::octree::render_asset::RenderOctree;
use crate::octree::visibility::extract::{ExtractOctreeNode, RenderOctreeNodes};
use crate::octree::visibility::VisibleOctreeNodes;
use bevy_app::{App, Plugin};
use bevy_asset::{AssetId, Assets};
use bevy_camera::visibility::ViewVisibility;
use bevy_camera::Camera;
use bevy_ecs::bundle::{Bundle, NoBundleEffect};
use bevy_ecs::prelude::*;
use bevy_ecs::query::{QueryFilter, QueryItem, ReadOnlyQueryData};
use bevy_ecs::system::{StaticSystemParam, SystemParam, SystemParamItem};
use bevy_log::prelude::*;
use bevy_platform::collections::hash_map::Entry;
use bevy_platform::collections::{HashMap, HashSet};
use bevy_reflect::TypePath;
use bevy_render::render_resource::AsBindGroupError;
use bevy_render::{Extract, MainWorld, RenderApp};
use std::sync::atomic::{AtomicUsize, Ordering};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PrepareOctreeNodeError<E: TypePath + Send + Sync + 'static> {
    #[error("Failed to prepare asset")]
    RetryNextUpdate(OctreeNode<E>),
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
pub trait RenderOctreeNode: Send + Sync + Sized + 'static + TypePath {
    /// The representation of the octree node in the "main world".
    type SourceOctreeNode: ExtractOctreeNode<Out = Self::ExtractedOctreeNode>;

    type ExtractedOctreeNode: TypePath + Sized + Send + Sync;

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
    fn byte_len(source_node: &OctreeNode<Self::ExtractedOctreeNode>) -> Option<usize> {
        None
    }

    /// Prepares the [`RenderAsset::SourceAsset`] for the GPU by transforming it into a [`RenderAsset`].
    ///
    /// ECS data may be accessed via `param`.
    fn prepare_octree_node(
        source_node: &OctreeNode<Self::ExtractedOctreeNode>,
        asset_id: AssetId<Octree<Self::SourceOctreeNode>>,
        node_id: NodeId,
        param: &mut SystemParamItem<Self::Param>,
    ) -> Result<Self, PrepareOctreeNodeError<Self::ExtractedOctreeNode>>;

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

/// All assets that should be prepared next frame.
#[derive(Resource)]
pub struct PrepareNextFrameOctreeNodes<A: RenderOctreeNode> {
    assets: Vec<(
        AssetId<Octree<A::SourceOctreeNode>>,
        OctreeNode<A::ExtractedOctreeNode>,
    )>,
}

impl<A: RenderOctreeNode> Default for PrepareNextFrameOctreeNodes<A> {
    fn default() -> Self {
        Self {
            assets: Default::default(),
        }
    }
}

/// Stores all GPU representations ([`RenderAsset`])
/// of [`RenderAsset::SourceAsset`] as long as they exist.
#[derive(Resource)]
pub struct RenderOctrees<A: RenderOctreeNode>(
    HashMap<AssetId<Octree<A::SourceOctreeNode>>, RenderOctree<A>>,
);

impl<A: RenderOctreeNode> Default for RenderOctrees<A> {
    fn default() -> Self {
        Self(HashMap::new())
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

/// This system prepares all assets of the corresponding [`RenderAsset::SourceAsset`] type
/// which where extracted this frame for the GPU.
pub fn prepare_assets<A: RenderOctreeNode>(
    mut extracted_assets: ResMut<RenderOctreeNodes<A::SourceOctreeNode>>,
    mut render_assets: ResMut<RenderOctrees<A>>,
    mut prepare_next_frame: ResMut<PrepareNextFrameOctreeNodes<A>>,
    param: StaticSystemParam<<A as RenderOctreeNode>::Param>,
    bpf: Res<RenderAssetBytesPerFrameLimiter>,
) {
    let mut wrote_asset_count = 0;

    let mut param = param.into_inner();
    let queued_assets = core::mem::take(&mut prepare_next_frame.assets);
    for (id, extracted_octree_node) in queued_assets {
        info!("processing queue asset for {}", A::type_path());
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

        let write_bytes = if let Some(size) = A::byte_len(&extracted_octree_node) {
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
            &extracted_octree_node,
            id,
            extracted_octree_node.id,
            &mut param,
        ) {
            Ok(prepared_octree_node) => {
                let render_octree_node = OctreeNode::<A> {
                    id: extracted_octree_node.id,
                    parent_id: extracted_octree_node.parent_id.clone(),
                    child_index: extracted_octree_node.child_index,
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
                prepare_next_frame.assets.push((id, extracted_data));
            }
            Err(PrepareOctreeNodeError::AsBindGroupError(e)) => {
                error!(
                    "{} Bind group construction failed: {e}",
                    core::any::type_name::<A>()
                );
            }
        }
    }

    // for removed in extracted_assets.removed_assets.drain() {
    //     render_assets.remove(removed);
    //     A::unload_asset(removed, &mut param);
    // }

    let mut prepared_octree_nodes = HashMap::new();

    for (id, extracted_octree_nodes) in extracted_assets.render_octrees.drain() {
        let mut prepared_nodes = Vec::new();

        let mut render_asset = match render_assets.get_mut(id) {
            None => {
                let render_octree = RenderOctree::<A>::default();
                render_assets.insert(id, render_octree);
                render_assets.get_mut(id).unwrap()
            }
            Some(asset) => asset,
        };

        for (node_id, extracted_octree_node) in extracted_octree_nodes.nodes {
            let write_bytes = if let Some(size) = A::byte_len(&extracted_octree_node) {
                if bpf.exhausted() {
                    prepare_next_frame.assets.push((id, extracted_octree_node));
                    continue;
                }
                size
            } else {
                0
            };

            match A::prepare_octree_node(
                &extracted_octree_node,
                id,
                extracted_octree_node.id,
                &mut param,
            ) {
                Ok(prepared_octree_node) => {
                    let render_octree_node = OctreeNode::<A> {
                        id: extracted_octree_node.id,
                        parent_id: extracted_octree_node.parent_id.clone(),
                        child_index: extracted_octree_node.child_index,
                        children: extracted_octree_node.children.clone(),
                        children_mask: extracted_octree_node.children_mask.clone(),
                        bounding_box: extracted_octree_node.bounding_box.clone(),
                        data: prepared_octree_node,
                    };
                    render_asset.insert(extracted_octree_node.id, render_octree_node);

                    bpf.write_bytes(write_bytes);
                    wrote_asset_count += 1;

                    prepared_nodes.push(node_id);
                }
                Err(PrepareOctreeNodeError::RetryNextUpdate(extracted_data)) => {
                    prepare_next_frame.assets.push((id, extracted_data));
                }
                Err(PrepareOctreeNodeError::AsBindGroupError(e)) => {
                    error!(
                        "{} Bind group construction failed: {e}",
                        core::any::type_name::<A>()
                    );
                }
            }
        }
        prepared_octree_nodes.insert(id, prepared_nodes);
    }

    // append the prepared octrees to the tracking structure
    for (id, nodes) in prepared_octree_nodes.drain() {
        let prepared_octrees = extracted_assets.prepared_octrees.entry(id).or_default();
        prepared_octrees.extend(nodes);
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
