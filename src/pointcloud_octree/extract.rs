use bevy_math::Vec3;
use bevy_reflect::TypePath;
use bevy_render::render_resource::{
    BindGroup, Buffer, PreparedBindGroup, ShaderType, UniformBuffer,
};
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

#[derive(TypePath)]
pub struct RenderPointCloudNodeUniform {
    pub prepared: PreparedBindGroup,
}
