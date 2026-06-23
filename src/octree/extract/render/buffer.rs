use std::marker::PhantomData;

use bevy_ecs::prelude::*;
use bevy_platform::collections::HashMap;
use bevy_render::{
    render_resource::{Buffer, BufferDescriptor, BufferUsages},
    renderer::{RenderDevice, RenderQueue},
};
use thiserror::Error;

use crate::octree::extract::render::asset::RenderOctreeNodeAllocation;

/// Describes how an octree node gets extracted and prepared for rendering.
///
/// In the [`ExtractSchedule`] step the [`RenderOctreeNode::SourceOctreeNode`] is transferred
/// from the "main world" into the "render world".
///
/// After that in the [`RenderSystems::PrepareAssets`] step the extracted octree nodes
/// are transformed into their GPU-representation of type [`RenderOctreeNode`].
pub trait RenderNodeData: Send + Sync {
    fn data(&self) -> &[u8];
}

/// Stores all GPU representations ([`RenderAsset`])
/// of [`RenderAsset::SourceAsset`] as long as they exist.
#[derive(Resource)]
pub struct ErasedRenderOctreesBuffers<ERA>(HashMap<usize, RenderOctreesBuffer<ERA>>);

impl<ERA> Default for ErasedRenderOctreesBuffers<ERA> {
    fn default() -> Self {
        Self(HashMap::new())
    }
}

impl<ERA> ErasedRenderOctreesBuffers<ERA> {
    pub fn get(&self, index: usize) -> Option<&RenderOctreesBuffer<ERA>> {
        self.0.get(&index)
    }

    pub fn get_or_insert_mut(
        &mut self,
        index: usize,
        render_device: &RenderDevice,
        size: u64,
    ) -> &mut RenderOctreesBuffer<ERA> {
        self.0
            .entry(index)
            .or_insert_with(|| RenderOctreesBuffer::<ERA>::new(render_device, size))
    }

    #[allow(unused)]
    pub fn remove(&mut self, index: usize) -> Option<RenderOctreesBuffer<ERA>> {
        self.0.remove(&index)
    }
}

// pub struct AllocationInfo {
//     pub allocation: Allocation,
//     pub start: u32,
//     pub end: u32,
// }

#[derive(Debug, Error)]
pub enum WriteOctreeNodeError {
    #[error("Node data buffer full")]
    BufferFull,
}

#[derive(Resource)]
pub struct RenderOctreesBuffer<ERA> {
    pub buffer: Buffer,
    // pub num_points: u64,
    // pub allocator: Allocator,
    // pub allocation_index: HashMap<NodeId, AllocationInfo>,
    phantom_data: PhantomData<fn() -> ERA>,
}

impl<ERA> RenderOctreesBuffer<ERA> {
    pub fn new(render_device: &RenderDevice, size: u64) -> Self {
        let buffer = render_device.create_buffer(&BufferDescriptor {
            label: Some("octree_data_buffer"),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            size,
            mapped_at_creation: false,
        });

        Self {
            buffer,
            // num_points: 0,
            // allocator: Allocator::new(max_instances),
            // allocation_index: HashMap::new(),
            phantom_data: PhantomData,
        }
    }

    pub fn write(
        &mut self,
        render_queue: &RenderQueue,
        // node_id: NodeId,
        data: &[u8],
        allocation: &RenderOctreeNodeAllocation,
    ) -> Result<(), WriteOctreeNodeError> {
        // do not reallocate the same node
        // if self.allocation_index.contains_key(&node_id) {
        //     // log it because it shouldn't happen
        //     bevy_log::warn!("Tried to allocate twice the same node");
        //     return Ok(());
        // }

        // let num_points = instances.len() as u32;

        // let Some(allocation) = self.allocator.allocate(num_points) else {
        //     let report = self.allocator.storage_report();
        //     bevy_log::info!("Unable to allocate {}, status: {:#?}", num_points, report);

        //     return Err(WriteOctreeNodeError::BufferFull);
        // };

        // let offset = allocation.offset as u64;
        // let node_size = num_points as u64;
        // let allocation_size = self.allocator.allocation_size(allocation) as u64;
        // let trim_size = allocation_size - node_size;

        // self.num_points = self.num_points.max(offset + allocation_size);

        // let instance_size = size_of::<A::InstanceData>() as u64;

        // bevy_log::debug!(
        //     "Allocated {} at offset {} with size {} (instance size = {})",
        //     node_size,
        //     allocation.offset,
        //     allocation_size,
        //     instance_size,
        // );

        // let data: &[u8] = bytemuck::cast_slice(instances);
        render_queue.write_buffer(&self.buffer, allocation.offset, data);

        // self.allocation_index.insert(
        //     node_id,
        //     AllocationInfo {
        //         allocation,
        //         start: offset as u32,
        //         end: (offset + node_size) as u32,
        //     },
        // );

        Ok(())
    }

    // pub fn free_node(&mut self, node_id: &NodeId) -> bool {
    //     let Some(allocation_info) = self.allocation_index.remove(node_id) else {
    //         return false;
    //     };

    //     self.allocator.free(allocation_info.allocation);

    //     return true;
    // }
}
