use bevy_math::Vec3;
use bevy_reflect::TypePath;
use bevy_render::render_resource::{
    BindGroup, Buffer, ShaderType, UniformBuffer,
};
use bytemuck::{Pod, Zeroable};

#[derive(ShaderType, Pod, Zeroable, Clone, Copy)]
#[repr(C)]
pub struct PointCloudNodeDataUniform {
    pub spacing: f32,
    pub level: u32,
    pub center: Vec3,
    pub half_extents: Vec3,
}

#[derive(ShaderType, Pod, Zeroable, Clone, Copy, Default)]
#[repr(C)]
pub struct PointCloudOctreeUniform {
    pub octree_index: u32,
    // WebGL2 structs must be 16 byte aligned.
    #[cfg(all(feature = "webgl", target_arch = "wasm32", not(feature = "webgpu")))]
    pub _webgl2_padding: Vec3,
}

#[derive(TypePath)]
pub struct RenderPointCloudNodeData {
    pub points: Option<Buffer>,
    pub uniform: BindGroup,
    pub uniform_buffer: UniformBuffer<PointCloudNodeDataUniform>,
    pub num_points: usize,
    pub offset: f32,
}
