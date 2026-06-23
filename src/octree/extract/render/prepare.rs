use std::marker::PhantomData;

use bevy_ecs::{prelude::*, system::StaticSystemParam};
use bevy_log::prelude::*;
use bevy_platform::collections::HashMap;
use bevy_render::{
    render_resource::{BindGroupEntries, UniformBuffer},
    renderer::{RenderDevice, RenderQueue},
};

use super::{
    super::{
        limiter::RenderOctreeNodesBytesPerFrameLimiter, ExtractedOctreeNodes, OctreeNodeExtraction,
    },
    asset::RenderOctreeNodeData,
    node::PrepareOctreeNodeError,
    resources::ErasedRenderOctrees,
};
use crate::octree::extract::render::{
    buffer::RenderNodeData,
    components::RenderOctreeEntityUniform,
    resources::{AllocatedOctreeNodes, OctreeEntityLayout, RenderOctreeIndex},
    uniforms::OctreeEntityUniform,
};

pub fn prepare_octrees_uniforms<E: OctreeNodeExtraction>(
    mut render_octree_index: ResMut<RenderOctreeIndex<E::Component>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    octree_entity_layout: Res<OctreeEntityLayout>,
    mut commands: Commands,
) {
    for entity in std::mem::take(&mut render_octree_index.added_octrees) {
        let Some(octree_index) = render_octree_index.octrees_index.get(&entity) else {
            warn!("Missing index for octree {}", entity);
            continue;
        };
        let mut octree_buffer = UniformBuffer::from(OctreeEntityUniform {
            octree_index: *octree_index as u32,
        });
        octree_buffer.write_buffer(&render_device, &render_queue);

        let bind_group = render_device.create_bind_group(
            "octree_entity_bind_group",
            &octree_entity_layout.bind_group_layout,
            &BindGroupEntries::single(&octree_buffer),
        );

        commands
            .entity(entity)
            .insert(RenderOctreeEntityUniform::<E::NodeData, E::Component> {
                bind_group,
                _phantom: PhantomData,
            });
    }
}

/// This system prepares all assets of the corresponding [`RenderAsset::SourceAsset`] type
/// which where extracted this frame for the GPU.
#[cfg_attr(feature = "trace", tracing::instrument(skip_all))]
#[allow(clippy::too_many_arguments)]
pub fn prepare_assets<E: OctreeNodeExtraction>(
    mut extracted_octree_nodes: ResMut<ExtractedOctreeNodes<E>>,
    mut allocated_octree_nodes: ResMut<AllocatedOctreeNodes<E::NodeData>>,
    mut render_octrees: ResMut<ErasedRenderOctrees<E::ErasedRenderOctreeNode>>,
    mut render_octrees_buffers: ResMut<
        super::buffer::ErasedRenderOctreesBuffers<E::ErasedRenderOctreeNode>,
    >,
    mut prepare_next_frame: ResMut<super::resources::PrepareNextFrameOctreeNodes<E>>,
    param: StaticSystemParam<E::PrepareParam>,
    bpf: Res<RenderOctreeNodesBytesPerFrameLimiter<E>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    // one single buffer for all octrees
    let octrees_buffer = render_octrees_buffers.get_or_insert_mut(
        0,
        &render_device,
        extracted_octree_nodes.buffer_size,
    );

    let mut wrote_asset_count = 0;

    let mut param = param.into_inner();
    let queued_assets = core::mem::take(&mut prepare_next_frame.assets);
    for (asset_id, extracted_octree_node) in queued_assets {
        if extracted_octree_nodes
            .removed_nodes
            .get(&asset_id)
            .map(|nodes| nodes.contains(&extracted_octree_node.id))
            .unwrap_or(false)
            || extracted_octree_nodes
                .added_nodes
                .get(&asset_id)
                .map(|nodes| nodes.contains(&extracted_octree_node.id))
                .unwrap_or(false)
        {
            // skip previous frame's assets that have been removed or updated
            continue;
        }

        let write_bytes = if let Some(size) = E::byte_len(&extracted_octree_node) {
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

        let render_asset = render_octrees.get_or_insert_mut(asset_id);

        // clone node metadata
        let cloned_node = RenderOctreeNodeData::<()> {
            id: extracted_octree_node.id,
            parent_id: extracted_octree_node.parent_id,
            child_index: extracted_octree_node.child_index,
            children: extracted_octree_node.children,
            children_mask: extracted_octree_node.children_mask,
            depth: extracted_octree_node.depth,
            bounding_box: extracted_octree_node.bounding_box,
            data: (),
            allocation: extracted_octree_node.allocation.clone(),
        };

        // write node data into buffer
        if let Err(error) = octrees_buffer.write(
            &render_queue,
            extracted_octree_node.data.data(),
            &extracted_octree_node.allocation,
        ) {
            bevy_log::warn!(
                "An error occured when write node data, try next frame: {:#}",
                error
            );
            prepare_next_frame
                .assets
                .push((asset_id, extracted_octree_node));
            continue;
        };

        // write allocation infos
        let allocated_nodes = allocated_octree_nodes.get_or_create_mut(asset_id);
        allocated_nodes.insert(cloned_node.id, extracted_octree_node.allocation.clone());

        match E::prepare_octree_node(extracted_octree_node, asset_id, &mut param) {
            Ok(prepared_octree_node) => {
                let render_octree_node = RenderOctreeNodeData::<E::ErasedRenderOctreeNode> {
                    id: cloned_node.id,
                    parent_id: cloned_node.parent_id,
                    child_index: cloned_node.child_index,
                    children: cloned_node.children,
                    children_mask: cloned_node.children_mask,
                    depth: cloned_node.depth,
                    bounding_box: cloned_node.bounding_box,
                    data: prepared_octree_node,
                    allocation: cloned_node.allocation,
                };

                render_asset.insert(cloned_node.id, render_octree_node);

                bpf.write_bytes(write_bytes);
                wrote_asset_count += 1;
            }
            Err(PrepareOctreeNodeError::RetryNextUpdate(extracted_data)) => {
                // try again next frame
                prepare_next_frame.assets.push((asset_id, extracted_data));
            }
            Err(PrepareOctreeNodeError::AsBindGroupError(e)) => {
                error!(
                    "{} Bind group construction failed: {e}",
                    core::any::type_name::<E>()
                );
                // TODO notify main world through a channel ?
            }
        }
    }

    // remove removed nodes from gpu
    for (asset_id, node_ids) in extracted_octree_nodes.removed_nodes.drain() {
        let render_octree = render_octrees.get_or_insert_mut(asset_id);
        let allocated_nodes = allocated_octree_nodes.get_or_create_mut(asset_id);

        for node_id in node_ids {
            render_octree.nodes.remove(&node_id);
            allocated_nodes.remove(&node_id);

            E::unload_octree_node(asset_id, node_id, &mut param);
        }
    }

    let mut prepared_octree_nodes = HashMap::new();

    for (asset_id, extracted_octree_nodes) in extracted_octree_nodes.octrees.drain() {
        let allocated_nodes = allocated_octree_nodes.get_or_create_mut(asset_id);

        let mut prepared_nodes = Vec::new();
        let render_asset = render_octrees.get_or_insert_mut(asset_id);

        for (node_id, extracted_octree_node) in extracted_octree_nodes {
            let write_bytes = if let Some(size) = E::byte_len(&extracted_octree_node) {
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
                parent_id: extracted_octree_node.parent_id,
                child_index: extracted_octree_node.child_index,
                children: extracted_octree_node.children,
                children_mask: extracted_octree_node.children_mask,
                depth: extracted_octree_node.depth,
                bounding_box: extracted_octree_node.bounding_box,
                data: (),
                allocation: extracted_octree_node.allocation.clone(),
            };

            // write node data into buffer
            if let Err(error) = octrees_buffer.write(
                &render_queue,
                extracted_octree_node.data.data(),
                &extracted_octree_node.allocation,
            ) {
                bevy_log::warn!(
                    "An error occured when write node data, try next frame: {:#}",
                    error
                );
                prepare_next_frame
                    .assets
                    .push((asset_id, extracted_octree_node));
                continue;
            };

            allocated_nodes.insert(cloned_node.id, extracted_octree_node.allocation.clone());

            match E::prepare_octree_node(extracted_octree_node, asset_id, &mut param) {
                Ok(prepared_octree_node) => {
                    let render_octree_node = RenderOctreeNodeData::<E::ErasedRenderOctreeNode> {
                        id: cloned_node.id,
                        parent_id: cloned_node.parent_id,
                        child_index: cloned_node.child_index,
                        children: cloned_node.children,
                        children_mask: cloned_node.children_mask,
                        depth: cloned_node.depth,
                        bounding_box: cloned_node.bounding_box,
                        data: prepared_octree_node,
                        allocation: cloned_node.allocation,
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
                        core::any::type_name::<E>()
                    );
                    // TODO notify main world ?
                }
            }
        }
        prepared_octree_nodes.insert(asset_id, prepared_nodes);
    }

    if bpf.exhausted() && !prepare_next_frame.assets.is_empty() {
        debug!(
            "{} write budget exhausted with {} assets remaining (wrote {})",
            core::any::type_name::<E>(),
            prepare_next_frame.assets.len(),
            wrote_asset_count
        );
    }
}
