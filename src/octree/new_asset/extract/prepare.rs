use super::super::node::{NodeData};
use super::limiter::RenderOctreeNodesBytesPerFrameLimiter;
use super::{ExtractedOctreeNodes, OctreeNodeExtraction};
use crate::octree::new_asset::asset::NewOctree;
use crate::octree::new_asset::extract::render_asset::RenderOctreeNodeData;
use bevy_asset::AssetId;
use bevy_ecs::prelude::*;
use bevy_ecs::system::{StaticSystemParam, SystemParam, SystemParamItem};
use bevy_log::prelude::*;
use bevy_platform::collections::HashMap;
use bevy_render::render_resource::AsBindGroupError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PrepareOctreeNodeError<T: Send + Sync> {
    #[error("Failed to prepare asset")]
    RetryNextUpdate(RenderOctreeNodeData<T>),
    #[error("Failed to build bind group: {0}")]
    AsBindGroupError(AsBindGroupError),
}

/// Describes how an octree node gets extracted and prepared for rendering.
///
/// In the [`ExtractSchedule`] step the [`RenderOctreeNode::SourceOctreeNode`] is transferred
/// from the "main world" into the "render world".
///
/// After that in the [`RenderSystems::PrepareAssets`] step the extracted octree nodes
/// are transformed into their GPU-representation of type [`RenderOctreeNode`].
pub trait RenderOctreeNode: Send + Sync + Sized + 'static {
    type SourceOctreeNode: NodeData;

    type ExtractedOctreeNode: NodeData + Sized;

    /// Specifies all ECS data required by [`RenderAsset::prepare_asset`].
    ///
    /// For convenience use the [`lifetimeless`](bevy_ecs::system::lifetimeless) [`SystemParam`].
    type Param: SystemParam;

    /// Size of the data the asset will upload to the gpu. Specifying a return value
    /// will allow the asset to be throttled via [`RenderOctreeNodesBytesPerFrame`].
    #[inline]
    #[expect(
        unused_variables,
        reason = "The parameters here are intentionally unused by the default implementation; however, putting underscores here will result in the underscores being copied by rust-analyzer's tab completion."
    )]
    fn byte_len(_source_node: &RenderOctreeNodeData<Self::ExtractedOctreeNode>) -> Option<usize> {
        None
    }

    /// Prepares the [`RenderAsset::SourceAsset`] for the GPU by transforming it into a [`RenderAsset`].
    ///
    /// ECS data may be accessed via `param`.
    fn prepare_octree_node(
        source_node: RenderOctreeNodeData<Self::ExtractedOctreeNode>,
        asset_id: AssetId<NewOctree<Self::SourceOctreeNode>>,
        param: &mut SystemParamItem<Self::Param>,
    ) -> Result<Self, PrepareOctreeNodeError<Self::ExtractedOctreeNode>>;

    /// Called whenever the [`RenderAsset::SourceAsset`] has been removed.
    ///
    /// You can implement this method if you need to access ECS data (via
    /// `_param`) in order to perform cleanup tasks when the asset is removed.
    ///
    /// The default implementation does nothing.
    fn unload_asset(
        _source_asset: AssetId<NewOctree<Self::SourceOctreeNode>>,
        _param: &mut SystemParamItem<Self::Param>,
    ) {
    }
}

/// This system prepares all assets of the corresponding [`RenderAsset::SourceAsset`] type
/// which where extracted this frame for the GPU.
pub fn prepare_assets<E, A>(
    mut extracted_assets: ResMut<ExtractedOctreeNodes<E>>,
    mut render_assets: ResMut<super::resources::RenderOctrees<A>>,
    mut prepare_next_frame: ResMut<super::resources::PrepareNextFrameOctreeNodes<A>>,
    param: StaticSystemParam<<A as RenderOctreeNode>::Param>,
    bpf: Res<RenderOctreeNodesBytesPerFrameLimiter>,
) where
    E: OctreeNodeExtraction,
    A: RenderOctreeNode<
            SourceOctreeNode = E::NodeData,
            ExtractedOctreeNode = E::ExtractedNodeData,
        >,
{
    let mut wrote_asset_count = 0;

    let mut param = param.into_inner();
    let queued_assets = core::mem::take(&mut prepare_next_frame.assets);
    for (asset_id, extracted_octree_node) in queued_assets {
        if extracted_assets
            .removed_nodes
            .get(&asset_id)
            .map(|nodes| nodes.contains(&extracted_octree_node.id))
            .unwrap_or(false)
            || extracted_assets
                .added_nodes
                .get(&asset_id)
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
                prepare_next_frame
                    .assets
                    .push((asset_id, extracted_octree_node));
                continue;
            }
            size
        } else {
            0
        };

        let render_asset = render_assets.get_or_insert_mut(asset_id);

        // clone node metadata
        let cloned_node = RenderOctreeNodeData::<()> {
            id: extracted_octree_node.id,
            parent_id: extracted_octree_node.parent_id.clone(),
            child_index: extracted_octree_node.child_index,
            children: extracted_octree_node.children.clone(),
            children_mask: extracted_octree_node.children_mask,
            depth: extracted_octree_node.depth,
            bounding_box: extracted_octree_node.bounding_box.clone(),
            data: (),
        };

        match A::prepare_octree_node(extracted_octree_node, asset_id, &mut param) {
            Ok(prepared_octree_node) => {
                let render_octree_node = RenderOctreeNodeData::<A> {
                    id: cloned_node.id,
                    parent_id: cloned_node.parent_id,
                    child_index: cloned_node.child_index,
                    children: cloned_node.children,
                    children_mask: cloned_node.children_mask,
                    depth: cloned_node.depth,
                    bounding_box: cloned_node.bounding_box,
                    data: prepared_octree_node,
                };
                render_asset.insert(cloned_node.id, render_octree_node);

                bpf.write_bytes(write_bytes);
                wrote_asset_count += 1;
            }
            Err(PrepareOctreeNodeError::RetryNextUpdate(extracted_data)) => {
                prepare_next_frame.assets.push((asset_id, extracted_data));
            }
            Err(PrepareOctreeNodeError::AsBindGroupError(e)) => {
                error!(
                    "{} Bind group construction failed: {e}",
                    core::any::type_name::<A>()
                );
            }
        }
    }

    // TODO cleanup
    // for removed in extracted_assets.removed_assets.drain() {
    //     render_assets.remove(removed);
    //     A::unload_asset(removed, &mut param);
    // }

    let mut prepared_octree_nodes = HashMap::new();

    for (asset_id, extracted_octree_nodes) in extracted_assets.octrees.drain() {
        let mut prepared_nodes = Vec::new();

        let render_asset = render_assets.get_or_insert_mut(asset_id);

        for (node_id, extracted_octree_node) in extracted_octree_nodes {
            let write_bytes = if let Some(size) = A::byte_len(&extracted_octree_node) {
                if bpf.exhausted() {
                    prepare_next_frame
                        .assets
                        .push((asset_id, extracted_octree_node));
                    continue;
                }
                size
            } else {
                0
            };

            // clone node metadata
            let cloned_node = RenderOctreeNodeData::<()> {
                id: extracted_octree_node.id,
                parent_id: extracted_octree_node.parent_id.clone(),
                child_index: extracted_octree_node.child_index,
                children: extracted_octree_node.children.clone(),
                children_mask: extracted_octree_node.children_mask,
                depth: extracted_octree_node.depth,
                bounding_box: extracted_octree_node.bounding_box.clone(),
                data: (),
            };

            match A::prepare_octree_node(extracted_octree_node, asset_id, &mut param) {
                Ok(prepared_octree_node) => {
                    let render_octree_node = RenderOctreeNodeData::<A> {
                        id: cloned_node.id,
                        parent_id: cloned_node.parent_id,
                        child_index: cloned_node.child_index,
                        children: cloned_node.children,
                        children_mask: cloned_node.children_mask,
                        depth: cloned_node.depth,
                        bounding_box: cloned_node.bounding_box,
                        data: prepared_octree_node,
                    };
                    render_asset.insert(cloned_node.id, render_octree_node);

                    bpf.write_bytes(write_bytes);
                    wrote_asset_count += 1;

                    prepared_nodes.push(node_id);
                }
                Err(PrepareOctreeNodeError::RetryNextUpdate(extracted_data)) => {
                    prepare_next_frame.assets.push((asset_id, extracted_data));
                }
                Err(PrepareOctreeNodeError::AsBindGroupError(e)) => {
                    error!(
                        "{} Bind group construction failed: {e}",
                        core::any::type_name::<A>()
                    );
                }
            }
        }
        prepared_octree_nodes.insert(asset_id, prepared_nodes);
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
