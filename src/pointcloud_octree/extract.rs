use bevy_ecs::prelude::*;
use bevy_math::Vec3;
use bevy_reflect::TypePath;
use bevy_render::render_resource::binding_types::uniform_buffer;
use bevy_render::render_resource::{
    BindGroup, BindGroupLayout, BindGroupLayoutEntries, Buffer, PreparedBindGroup, ShaderStages, ShaderType, UniformBuffer,
};
use bevy_render::renderer::RenderDevice;
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
    pub points: Option<Buffer>,
    pub uniform: BindGroup,
    pub uniform_buffer: UniformBuffer<PointCloudNodeDataUniform>,
    pub num_points: usize,
    pub offset: f32,
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
