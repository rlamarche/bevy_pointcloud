use crate::point_cloud::{QUAD_INDICES, QUAD_POSITIONS};
use bevy_ecs::prelude::*;
use bevy_mesh::VertexBufferLayout;
use bevy_render::render_resource::{
    Buffer, BufferInitDescriptor, BufferUsages, VertexAttribute, VertexFormat, VertexStepMode,
};
use bevy_render::renderer::RenderDevice;

#[derive(Resource)]
pub struct PointCloudOctreeMesh {
    pub vertex_buffer: Buffer,
    pub vertex_buffer_layout: VertexBufferLayout,
    pub index_buffer: Buffer,
    pub index_count: u32,
}

impl FromWorld for PointCloudOctreeMesh {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let vertex_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("pcl_octree_mesh_vertex_buffer"),
            contents: bytemuck::cast_slice(QUAD_POSITIONS),
            usage: BufferUsages::VERTEX,
        });

        let index_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("pcl_octree_mesh_index_buffer"),
            contents: bytemuck::cast_slice(QUAD_INDICES),
            usage: BufferUsages::INDEX,
        });

        let vertex_buffer_layout = VertexBufferLayout {
            array_stride: VertexFormat::Float32x3.size(), // 12 bytes
            step_mode: VertexStepMode::Vertex,
            attributes: vec![VertexAttribute {
                format: VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0, // @location(0) dans le shader
            }],
        };

        Self {
            vertex_buffer,
            vertex_buffer_layout,
            index_buffer,
            index_count: QUAD_INDICES.len() as u32,
        }
    }
}
