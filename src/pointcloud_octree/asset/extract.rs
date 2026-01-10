use crate::octree::extract::OctreeNodeExtraction;
use crate::octree::extract::prepare::{PrepareOctreeNodeError, RenderOctreeNode};
use crate::octree::node::OctreeNode;
use crate::pointcloud_octree::asset::data::{PointCloudNodeData, PointData};
use crate::pointcloud_octree::extract::{PointCloudNodeDataUniform, RenderPointCloudNodeData};

use crate::octree::asset::Octree;
use crate::octree::extract::render_asset::RenderOctreeNodeData;
use crate::pointcloud_octree::component::PointCloudOctree3d;
use bevy_asset::AssetId;
use bevy_ecs::query::QueryItem;
use bevy_ecs::{
    prelude::*,
    system::{SystemParamItem, lifetimeless::SRes},
};
use bevy_reflect::TypePath;
use bevy_render::render_resource::binding_types::uniform_buffer;
use bevy_render::render_resource::{
    BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries, Buffer, BufferInitDescriptor,
    BufferUsages, PreparedBindGroup, ShaderStages, ShaderType, UniformBuffer,
};
use bevy_render::renderer::{RenderDevice, RenderQueue};
use bytemuck::{Pod, Zeroable};

#[derive(TypePath)]
pub struct PointCloudOctreeExtraction;

impl OctreeNodeExtraction for PointCloudOctreeExtraction {
    type QueryData = ();
    type QueryFilter = ();
    type NodeData = PointCloudNodeData;
    type Component = PointCloudOctree3d;
    type ExtractedNodeData = PointCloudNodeData;

    fn extract_octree_node(
        node: &OctreeNode<Self::NodeData>,
        _item: &QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::ExtractedNodeData> {
        node.data.clone()
    }
}

#[derive(Resource)]
pub struct PointCloudOctreeNodeUniformLayout {
    pub layout: BindGroupLayout,
}

impl FromWorld for PointCloudOctreeNodeUniformLayout {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        Self {
            layout: render_device.create_bind_group_layout(
                "pcl_octree_node_data",
                &BindGroupLayoutEntries::single(
                    ShaderStages::VERTEX,
                    uniform_buffer::<PointCloudNodeDataUniform>(false),
                ),
            ),
        }
    }
}

impl RenderOctreeNode for RenderPointCloudNodeData {
    type SourceOctreeNode = PointCloudNodeData;
    type ExtractedOctreeNode = PointCloudNodeData;
    type Param = (
        SRes<RenderDevice>,
        SRes<RenderQueue>,
        SRes<PointCloudOctreeNodeUniformLayout>,
    );

    fn byte_len(source_node: &RenderOctreeNodeData<Self::ExtractedOctreeNode>) -> Option<usize> {
        Some(source_node.data.num_points * size_of::<PointData>())
    }

    fn prepare_octree_node(
        source_node: RenderOctreeNodeData<Self::ExtractedOctreeNode>,
        _asset_id: AssetId<Octree<Self::SourceOctreeNode>>,
        (render_device, render_queue, point_cloud_octree_node_uniform_layout): &mut SystemParamItem<
            Self::Param,
        >,
    ) -> Result<Self, PrepareOctreeNodeError<Self::ExtractedOctreeNode>> {
        let buffer = if source_node.data.points.len() > 0 {
            Some(
                render_device.create_buffer_with_data(&BufferInitDescriptor {
                    label: Some("PointCloud data buffer"),
                    contents: bytemuck::cast_slice(source_node.data.points.as_slice()),
                    usage: BufferUsages::VERTEX,
                }),
            )
        } else {
            None
        };

        let mut uniform_buffer = UniformBuffer::from(PointCloudNodeDataUniform {
            spacing: source_node.data.spacing,
            level: source_node.data.level,
            center: source_node.bounding_box.center.into(),
            half_extents: source_node.bounding_box.half_extents.into(),
            octree_index: 0,
            node_index: 0,
        });

        uniform_buffer.write_buffer(render_device, render_queue);

        let uniform = render_device.create_bind_group(
            "pcl_pointcloud_octree_node_data",
            &point_cloud_octree_node_uniform_layout.layout,
            &BindGroupEntries::single(uniform_buffer.binding().unwrap()),
        );

        Ok(RenderPointCloudNodeData {
            points: buffer,
            uniform,
            uniform_buffer,
            num_points: source_node.data.num_points,
            offset: source_node.data.offset,
        })
    }
}
