use crate::octree::extract::{RenderOctreeIndex, RenderOctrees};
use crate::octree::storage::NodeId;
use crate::pointcloud_octree::component::PointCloudOctree3d;
use crate::pointcloud_octree::extract::RenderPointCloudNodeData;
use crate::pointcloud_octree::render::phase::PointCloudOctreeBinnedPhaseItem;
use bevy_ecs::prelude::*;
use bevy_ecs::query::ROQueryItem;
use bevy_ecs::system::SystemParamItem;
use bevy_ecs::system::lifetimeless::{Read, SRes};
use bevy_log::warn;
use bevy_platform::collections::HashMap;
use bevy_render::render_phase::{RenderCommand, RenderCommandResult, TrackedRenderPass};
use bevy_render::render_resource::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource, BindingType,
    BufferBinding, BufferBindingType, BufferInitDescriptor, BufferSize, BufferUsages, Extent3d,
    ShaderStages,
};
use bevy_render::renderer::RenderDevice;
use bevy_render::texture::ColorAttachment;
use bytemuck::{Pod, Zeroable};

/// Stores visible nodes and mapping textures for each view
#[derive(Component)]
pub struct VisibleNodesTexture {
    pub visible_nodes: Option<ColorAttachment>,
    pub size: Extent3d,
    /// contains node index per octree index (see [`RenderOctreeIndex`])
    pub node_index: Vec<HashMap<NodeId, u32>>,
}

const BUFFER_SIZE: usize = 65536;
pub const MAX_NODES: usize = 2048;

#[derive(Resource)]
pub struct OctreeNodesMappingBindGroups {
    pub layout: BindGroupLayout,
    // bind groups for each octree indexes
    pub bind_groups: Vec<Vec<BindGroup>>,
    pub min_uniform_buffer_offset_alignment: usize,
    pub max_nodes_per_buffer: usize,
    pub nb_buffers: usize,
}

impl FromWorld for OctreeNodesMappingBindGroups {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let layout = render_device.create_bind_group_layout(
            Some("layout_node_mapping"),
            &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: BufferSize::new(64),
                },
                count: None,
            }],
        );

        let min_uniform_buffer_offset_alignment = render_device
            .wgpu_device()
            .limits()
            .min_uniform_buffer_offset_alignment
            as usize
            * 2; // for android compat we multiply by 2
        // TODO: create a special case ?
        let max_nodes_per_buffer = BUFFER_SIZE / min_uniform_buffer_offset_alignment;
        let nb_buffers = MAX_NODES / max_nodes_per_buffer;

        Self {
            layout,
            bind_groups: Vec::new(),
            min_uniform_buffer_offset_alignment,
            max_nodes_per_buffer,
            nb_buffers,
        }
    }
}

/// The octree node mapping uniform layout
#[derive(Clone, Debug, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct OctreeNodeMapping {
    pub octree_index: u32,
    pub node_index: u32,
    pub _padding: [u8; 56],
}

pub fn prepare_octree_nodes_mapping_buffers(
    render_octree_index: Res<RenderOctreeIndex<PointCloudOctree3d>>,
    mut mappings: ResMut<OctreeNodesMappingBindGroups>,
    render_device: Res<RenderDevice>,
) {
    // add missing octree buffers
    for i in mappings.bind_groups.len()..render_octree_index.octrees_slab.len() {
        let mut buffer_data: Vec<u8> = Vec::with_capacity(BUFFER_SIZE * mappings.nb_buffers);

        for node_idx in 0..MAX_NODES {
            let data = [i as u32, node_idx as u32];

            buffer_data.extend_from_slice(bytemuck::cast_slice(&data));
            buffer_data.resize(
                buffer_data.len() + (mappings.min_uniform_buffer_offset_alignment - 8),
                0,
            );
        }

        let mut bind_groups = Vec::with_capacity(mappings.nb_buffers);
        for j in 0..mappings.nb_buffers {
            let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("node_mapping_buffer"),
                contents: &buffer_data[j * BUFFER_SIZE..(j + 1) * BUFFER_SIZE],
                usage: BufferUsages::UNIFORM,
            });
            let bind_group = render_device.create_bind_group(
                "bind_group_node_mapping",
                &mappings.layout,
                &[BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &buffer,
                        offset: 0,
                        size: BufferSize::new(mappings.min_uniform_buffer_offset_alignment as u64),
                    }),
                }],
            );

            bind_groups.push(bind_group);
        }

        mappings.bind_groups.push(bind_groups);
    }
}

pub struct SetVisibleOctreeUniformGroup<const I: usize>;
impl<P: PointCloudOctreeBinnedPhaseItem, const I: usize> RenderCommand<P>
    for SetVisibleOctreeUniformGroup<I>
{
    type Param = (
        SRes<RenderOctreeIndex<PointCloudOctree3d>>,
        SRes<RenderOctrees<RenderPointCloudNodeData>>,
        SRes<OctreeNodesMappingBindGroups>,
    );
    type ViewQuery = Read<VisibleNodesTexture>;
    type ItemQuery = Read<PointCloudOctree3d>;

    fn render<'w>(
        item: &P,
        visible_nodes: ROQueryItem<'w, '_, Self::ViewQuery>,
        point_cloud_octree_3d: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        (render_octree_index, render_octrees, octree_nodes_mapping_bind_groups): SystemParamItem<
            'w,
            '_,
            Self::Param,
        >,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let render_octree_index = render_octree_index.into_inner();
        let render_octrees = render_octrees.into_inner();
        let octree_nodes_mapping_bind_groups = octree_nodes_mapping_bind_groups.into_inner();

        let Some(point_cloud_octree_3d) = point_cloud_octree_3d else {
            warn!("Missing point cloud octree 3d item");
            return RenderCommandResult::Skip;
        };

        let Some(_octree) = render_octrees.get(point_cloud_octree_3d) else {
            debug!("Missing octree when render");
            return RenderCommandResult::Skip;
        };

        let Some(octree_index) = render_octree_index.get_octree_index(item.entity()) else {
            debug!("Missing octree when render");
            return RenderCommandResult::Skip;
        };

        // get the octree's bind group
        let bind_group = &octree_nodes_mapping_bind_groups.bind_groups[octree_index];

        let node_ids = item.node_ids();

        if node_ids.len() == 0 {
            return RenderCommandResult::Skip;
        }

        let node_id = node_ids[0];

        // get the node's index
        let Some(node_index) = visible_nodes.node_index[octree_index].get(&node_id) else {
            // warn!("Missing node index when render");
            // it happens when there is more than 2048 nodes to render
            return RenderCommandResult::Skip;
        };

        // we are ready to bind the correct bind group with the correct dynamic offset

        let buffer_index =
            *node_index as usize / octree_nodes_mapping_bind_groups.max_nodes_per_buffer;
        let remapped_node_index =
            *node_index % octree_nodes_mapping_bind_groups.max_nodes_per_buffer as u32;

        let min_uniform_buffer_offset_alignment =
            octree_nodes_mapping_bind_groups.min_uniform_buffer_offset_alignment as u32;

        pass.set_bind_group(
            I,
            &bind_group[buffer_index],
            &[remapped_node_index * min_uniform_buffer_offset_alignment],
        );

        RenderCommandResult::Success
    }
}
