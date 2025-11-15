use super::asset::{PointCloudNodeData, PointData};
use crate::octree::asset::{Octree, OctreeNode};
use crate::octree::visibility::extract::ExtractOctreeNode;
use crate::octree::visibility::prepare::{PrepareOctreeNodeError, RenderOctreeNode};
use crate::point_cloud_material::PointCloudMaterial3d;
use crate::pointcloud_octree::component::PointCloudOctree3d;
use crate::pointcloud_octree::render::data::PointCloudOctree3dUniform;
use bevy_asset::AssetId;
use bevy_ecs::query::QueryItem;
use bevy_ecs::{
    prelude::*,
    system::{lifetimeless::SRes, SystemParamItem},
};
use bevy_math::Vec3;
use bevy_reflect::TypePath;
use bevy_render::extract_component::ExtractComponent;
use bevy_render::render_resource::binding_types::uniform_buffer;
use bevy_render::render_resource::{BindGroup, BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries, Buffer, BufferInitDescriptor, BufferUsages, PreparedBindGroup, ShaderStages, ShaderType, UniformBuffer};
use bevy_render::renderer::{RenderDevice, RenderQueue};
use bevy_transform::prelude::GlobalTransform;
use bytemuck::{Pod, Zeroable};

#[derive(ShaderType, Pod, Zeroable, Clone, Copy)]
#[repr(C)]
pub struct PointCloudNodeDataUniform {
    pub spacing: f32,
    pub level: u32,
    pub center: Vec3,
    pub half_extents: Vec3,
    pub octree_index: u32,
    pub node_index: u32,
}

#[derive(TypePath)]
pub struct RenderPointCloudNodeData {
    pub points: Buffer,
    pub uniform: BindGroup,
    pub uniform_buffer: UniformBuffer<PointCloudNodeDataUniform>,
    pub num_points: usize,
    pub offset: f32,
}

impl RenderOctreeNode for RenderPointCloudNodeData {
    type SourceOctreeNode = PointCloudNodeData;
    type ExtractedOctreeNode = PointCloudNodeData;
    type Param = (
        SRes<RenderDevice>,
        SRes<RenderQueue>,
        SRes<PointCloudOctreeNodeUniformLayout>,
    );

    fn byte_len(source_node: &OctreeNode<Self::ExtractedOctreeNode>) -> Option<usize> {
        Some(source_node.data.num_points * size_of::<PointData>())
    }

    fn prepare_octree_node(
        source_node: OctreeNode<Self::ExtractedOctreeNode>,
        _asset_id: AssetId<Octree<Self::SourceOctreeNode>>,
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

#[derive(TypePath)]
pub struct RenderPointCloudNodeUniform {
    pub prepared: PreparedBindGroup,
}

impl ExtractOctreeNode for PointCloudNodeData {
    type QueryData = ();
    type QueryFilter = ();
    type Out = Self;

    fn extract_octree_node(
        node: &OctreeNode<Self>,
        _: &QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        Some(node.data.clone())
    }
}

impl ExtractComponent for PointCloudOctree3d {
    type QueryData = (
        &'static PointCloudOctree3d,
        &'static GlobalTransform,
        &'static PointCloudMaterial3d,
    );
    type QueryFilter = ();
    type Out = (
        PointCloudOctree3d,
        PointCloudOctree3dUniform,
        PointCloudMaterial3d,
    );

    fn extract_component(
        (point_cloud_3d, global_transform, point_cloud_material_3d): QueryItem<
            '_,
            '_,
            Self::QueryData,
        >,
    ) -> Option<Self::Out> {
        let point_cloud_octree_3d_uniform = PointCloudOctree3dUniform {
            world_from_local: global_transform.to_matrix(),
        };
        Some((
            point_cloud_3d.clone(),
            point_cloud_octree_3d_uniform,
            point_cloud_material_3d.clone(),
        ))
    }
}
