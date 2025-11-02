use super::asset::{PointCloudNodeData, PointCloudOctree, PointData};
use crate::octree::asset::{NodeId, Octree};
use crate::octree::extract::{PrepareOctreeNodeError, RenderOctreeNode};
use bevy_asset::AssetId;
use bevy_ecs::{
    prelude::*,
    system::{lifetimeless::SRes, SystemParamItem},
};
use bevy_ecs::query::QueryItem;
use bevy_log::info;
use bevy_reflect::TypePath;
use bevy_render::extract_component::ExtractComponent;
use bevy_render::render_resource::{Buffer, BufferInitDescriptor, BufferUsages};
use bevy_render::renderer::RenderDevice;
use bevy_transform::prelude::GlobalTransform;
use crate::point_cloud_material::PointCloudMaterial3d;
use crate::pointcloud_octree::component::PointCloudOctree3d;
use crate::pointcloud_octree::render::data::PointCloudOctree3dUniform;
use crate::pointcloud_octree::visibility::VisiblePointCloudOctree3dNodes;

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
        asset_id: AssetId<Octree<Self::SourceOctreeNode>>,
        node_id: NodeId,
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

// pub type RenderPointCloudOctree = Octree<RenderPointCloudNodeData>;

// #[derive(Debug, Clone, TypePath)]
// pub struct RenderPointCloudNodeData {
//     pub spacing: f32,
//     pub level: u32,
//     pub num_points: usize,
//     pub points: Buffer,
// }


impl ExtractComponent for PointCloudOctree3d {
    type QueryData = (
        &'static PointCloudOctree3d,
        &'static GlobalTransform,
        &'static PointCloudMaterial3d,
    );
    type QueryFilter = ();
    type Out = (PointCloudOctree3d, PointCloudOctree3dUniform, PointCloudMaterial3d);

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
