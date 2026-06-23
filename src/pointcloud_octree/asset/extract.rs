use std::marker::PhantomData;

use bevy_asset::AssetId;
use bevy_ecs::{
    prelude::*,
    system::{lifetimeless::SRes, SystemParamItem},
};
use bevy_reflect::TypePath;
use bevy_render::{
    render_resource::{
        binding_types::uniform_buffer, BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries,
        ShaderStages, UniformBuffer,
    },
    renderer::{RenderDevice, RenderQueue},
};

use crate::{
    octree::{
        asset::Octree,
        extract::{
            render::{asset::RenderOctreeNodeData, node::PrepareOctreeNodeError},
            OctreeNodeExtraction,
        },
        node::OctreeNode,
    },
    point::Point,
    point_cloud::{PointCloud, PointCloudGpuMapper},
    pointcloud_octree::{
        asset::data::PointCloudNodeData,
        component::PointCloudOctree3d,
        extract::{
            ErasedRenderPointCloudNode, PointCloudNodeDataUniform, RenderPointCloudNodeData,
        },
    },
};

#[derive(TypePath)]
pub struct PointCloudOctreeExtraction<T: Point>(PhantomData<fn() -> T>);

impl<T: Point> Default for PointCloudOctreeExtraction<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

#[derive(TypePath)]
pub struct PointCloudOctreeGpuMapper<A: PointCloudGpuMapper> {
    phantom: PhantomData<fn() -> A>,
}

impl<A: PointCloudGpuMapper> Default for PointCloudOctreeGpuMapper<A> {
    fn default() -> Self {
        Self {
            phantom: Default::default(),
        }
    }
}

impl<A: PointCloudGpuMapper> OctreeNodeExtraction for PointCloudOctreeGpuMapper<A> {
    type NodeData = PointCloudNodeData<A::Point>;
    type GpuData = A::GpuPoint;
    type Component = PointCloudOctree3d<A::Point>;
    type ExtractedNodeData = RenderPointCloudNodeData;
    type ExtractParam = A::Param;
    type PrepareParam = (
        SRes<RenderDevice>,
        SRes<RenderQueue>,
        SRes<PointCloudOctreeNodeUniformLayout>,
    );
    type ErasedRenderOctreeNode = ErasedRenderPointCloudNode;

    fn extract_octree_node(
        asset: &Octree<Self::NodeData>,
        node: &OctreeNode<Self::NodeData>,
        param: &mut SystemParamItem<Self::ExtractParam>,
    ) -> Result<Option<Self::ExtractedNodeData>, BevyError> {
        if let Some(data) = &node.data {
            let point_cloud = PointCloud {
                points: data.points.clone(),
                aabb: asset.node_root().map(|root| root.hierarchy.bounding_box),
            };
            let points = match A::convert(point_cloud, param) {
                Ok(points) => points,
                Err(e) => return Err(BevyError::from(e.to_string())),
            };
            Ok(Some(RenderPointCloudNodeData {
                spacing: data.spacing,
                level: data.level,
                offset: data.offset,
                point_count: data.point_count,
                buffer: bytemuck::cast_slice(points.as_slice()).into(),
            }))
        } else {
            Ok(None)
        }
    }

    fn prepare_octree_node(
        source_node: RenderOctreeNodeData<Self::ExtractedNodeData>,
        _asset_id: AssetId<Octree<Self::NodeData>>,
        (render_device, render_queue, point_cloud_octree_node_uniform_layout): &mut SystemParamItem<
            Self::PrepareParam,
        >,
    ) -> Result<Self::ErasedRenderOctreeNode, PrepareOctreeNodeError<Self::ExtractedNodeData>> {
        // TODO add option to use individual buffers and convert it here
        // let buffer = if source_node.data.points.len() > 0 {
        //     Some(
        //         render_device.create_buffer_with_data(&BufferInitDescriptor {
        //             label: Some("PointCloud data buffer"),
        //             contents: bytemuck::cast_slice(source_node.data.points.as_slice()),
        //             usage: BufferUsages::VERTEX,
        //         }),
        //     )
        // } else {
        //     None
        // };

        let mut uniform_buffer = UniformBuffer::from(PointCloudNodeDataUniform {
            spacing: source_node.data.spacing,
            level: source_node.data.level,
            center: source_node.bounding_box.center.into(),
            half_extents: source_node.bounding_box.half_extents.into(),
        });

        uniform_buffer.write_buffer(render_device, render_queue);

        let uniform = render_device.create_bind_group(
            "pcl_pointcloud_octree_node_data",
            &point_cloud_octree_node_uniform_layout.layout,
            &BindGroupEntries::single(uniform_buffer.binding().unwrap()),
        );

        Ok(ErasedRenderPointCloudNode {
            // points: buffer,
            points: None,
            uniform,
            uniform_buffer,
            num_points: source_node.data.point_count,
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
