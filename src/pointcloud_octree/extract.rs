use super::asset::{PointCloudNodeData, PointCloudOctree, PointData};
use crate::octree::asset::{NodeId, Octree, OctreeNode};
use crate::octree::extract::{PrepareOctreeNodeError, RenderOctreeNode};
use crate::octree::visibility::extract::ExtractOctreeNode;
use crate::point_cloud_material::PointCloudMaterial3d;
use crate::pointcloud_octree::component::PointCloudOctree3d;
use crate::pointcloud_octree::render::data::PointCloudOctree3dUniform;
use bevy_asset::AssetId;
use bevy_ecs::query::QueryItem;
use bevy_ecs::{
    prelude::*,
    system::{SystemParamItem, lifetimeless::SRes},
};
use bevy_log::info;
use bevy_reflect::TypePath;
use bevy_render::extract_component::ExtractComponent;
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_resource::{
    AsBindGroup, BindGroupLayout, Buffer, BufferInitDescriptor, BufferUsages, PreparedBindGroup,
};
use bevy_render::renderer::RenderDevice;
use bevy_transform::prelude::GlobalTransform;

#[derive(Clone, Debug, TypePath)]
pub struct RenderPointCloudNodeData {
    pub points: Buffer,
    pub num_points: usize,
}

impl RenderOctreeNode for RenderPointCloudNodeData {
    type SourceOctreeNode = PointCloudNodeData;
    type Param = SRes<RenderDevice>;

    fn byte_len(source_node: &Self::SourceOctreeNode) -> Option<usize> {
        Some(source_node.points.len() * size_of::<PointData>())
    }

    fn prepare_octree_node(
        source_node: Self::SourceOctreeNode,
        _asset_id: AssetId<Octree<Self::SourceOctreeNode>>,
        _node_id: NodeId,
        render_device: &mut SystemParamItem<Self::Param>,
    ) -> Result<Self, PrepareOctreeNodeError<Self::SourceOctreeNode>> {
        info!("Preparing octree node");

        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("PointCloud data buffer"),
            contents: bytemuck::cast_slice(source_node.points.as_slice()),
            usage: BufferUsages::VERTEX,
        });

        Ok(RenderPointCloudNodeData {
            points: buffer,
            num_points: source_node.num_points,
        })
    }
}

impl crate::octree::visibility::prepare::RenderOctreeNode for RenderPointCloudNodeData {
    type SourceOctreeNode = PointCloudNodeData;
    type ExtractedOctreeNode = PointCloudNodeData;
    type Param = SRes<RenderDevice>;

    fn byte_len(source_node: &OctreeNode<Self::ExtractedOctreeNode>) -> Option<usize> {
        Some(source_node.data.num_points * size_of::<PointData>())
    }

    fn prepare_octree_node(
        source_node: &OctreeNode<Self::ExtractedOctreeNode>,
        asset_id: AssetId<Octree<Self::SourceOctreeNode>>,
        node_id: NodeId,
        render_device: &mut SystemParamItem<Self::Param>,
    ) -> Result<
        Self,
        crate::octree::visibility::prepare::PrepareOctreeNodeError<Self::ExtractedOctreeNode>,
    > {
        info!("Preparing octree node 2.0");

        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("PointCloud data buffer"),
            contents: bytemuck::cast_slice(source_node.data.points.as_slice()),
            usage: BufferUsages::VERTEX,
        });

        Ok(RenderPointCloudNodeData {
            points: buffer,
            num_points: source_node.data.num_points,
        })
    }
}

#[derive(Resource)]
pub struct PointCloudNodeUniformLayout {
    pub layout: BindGroupLayout,
}

impl FromWorld for PointCloudNodeUniformLayout {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        Self {
            layout: PointCloudNodeData::bind_group_layout(render_device),
        }
    }
}

#[derive(TypePath)]
pub struct RenderPointCloudNodeUniform {
    pub prepared: PreparedBindGroup,
}

impl RenderOctreeNode for RenderPointCloudNodeUniform {
    type SourceOctreeNode = PointCloudNodeData;
    type Param = (
        SRes<RenderDevice>,
        SRes<PointCloudNodeUniformLayout>,
        (
            SRes<RenderAssets<bevy_render::texture::GpuImage>>,
            SRes<bevy_render::texture::FallbackImage>,
            SRes<RenderAssets<bevy_render::storage::GpuShaderStorageBuffer>>,
        ),
    );

    fn byte_len(source_node: &Self::SourceOctreeNode) -> Option<usize> {
        // f32 + u32 = 8
        Some(8)
    }

    fn prepare_octree_node(
        source_node: Self::SourceOctreeNode,
        _asset_id: AssetId<Octree<Self::SourceOctreeNode>>,
        _node_id: NodeId,
        (render_device, point_cloud_node_uniform_layout, materials): &mut SystemParamItem<
            Self::Param,
        >,
    ) -> Result<Self, PrepareOctreeNodeError<Self::SourceOctreeNode>> {
        info!("Preparing octree node uniform");

        let prepared = source_node
            .as_bind_group(
                &point_cloud_node_uniform_layout.layout,
                render_device,
                materials,
            )
            .map_err(|err| PrepareOctreeNodeError::AsBindGroupError(err))?;

        Ok(RenderPointCloudNodeUniform { prepared })
    }
}

impl ExtractOctreeNode for PointCloudNodeData {
    type QueryData = ();
    type QueryFilter = ();
    type Out = Self;

    fn extract_octree_node(
        node: &OctreeNode<Self>,
        _: &QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        info!("Extracting octree node 2.0");

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
