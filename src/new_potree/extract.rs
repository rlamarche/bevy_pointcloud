use crate::new_potree::component::NewPotreePointCloud3d;
use crate::new_potree::loader::PotreeHierarchy;
use crate::octree::new_asset::extract::OctreeNodeExtraction;
use crate::octree::new_asset::extract::prepare::{PrepareOctreeNodeError, RenderOctreeNode};
use crate::octree::new_asset::node::OctreeNode;
use crate::pointcloud_octree::asset::{PointCloudNodeData, PointData};
use crate::pointcloud_octree::extract::{PointCloudNodeDataUniform, PointCloudOctreeNodeUniformLayout, RenderPointCloudNodeData};

use bevy_asset::AssetId;
use bevy_ecs::query::QueryItem;
use bevy_ecs::{
    prelude::*,
    system::{SystemParamItem, lifetimeless::SRes},
};
use bevy_math::Vec3;
use bevy_reflect::TypePath;
use bevy_render::extract_component::ExtractComponent;
use bevy_render::render_resource::binding_types::uniform_buffer;
use bevy_render::render_resource::{
    BindGroup, BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries, Buffer,
    BufferInitDescriptor, BufferUsages, PreparedBindGroup, ShaderStages, ShaderType, UniformBuffer,
};
use bevy_render::renderer::{RenderDevice, RenderQueue};
use bevy_transform::prelude::GlobalTransform;
use bytemuck::{Pod, Zeroable};
use crate::octree::new_asset::asset::NewOctree;
use crate::octree::new_asset::extract::render_asset::RenderOctreeNodeData;

#[derive(TypePath)]
pub struct PotreeExtraction;

impl OctreeNodeExtraction for PotreeExtraction {
    type QueryData = ();
    type QueryFilter = ();
    type NodeData = PointCloudNodeData;
    type NodeHierarchy = PotreeHierarchy;
    type Component = NewPotreePointCloud3d;
    type ExtractedNodeData = PointCloudNodeData;

    fn extract_octree_node(
        node: &OctreeNode<Self::NodeHierarchy, Self::NodeData>,
        _item: &QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::ExtractedNodeData> {
        node.data.clone()
    }
}

impl RenderOctreeNode for RenderPointCloudNodeData {
    type SourceOctreeHierarchy = PotreeHierarchy;
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
        _asset_id: AssetId<NewOctree<Self::SourceOctreeHierarchy, Self::SourceOctreeNode>>,
        (render_device, render_queue, point_cloud_octree_node_uniform_layout): &mut SystemParamItem<
            Self::Param,
        >,
    ) -> Result<Self, PrepareOctreeNodeError<Self::ExtractedOctreeNode>> {
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("PointCloud data buffer"),
            contents: bytemuck::cast_slice(source_node.data.points.as_slice()),
            usage: BufferUsages::VERTEX,
        });

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
