use bevy_ecs::prelude::*;
use bevy_render::{
    render_resource::{Buffer, BufferInitDescriptor, BufferUsages},
    renderer::RenderDevice,
};

// pub const QUAD_POSITIONS: &[[f32; 4]] = &[
//     [-0.5, -0.5, 0.0, 1.0],
//     [0.5, -0.5, 0.0, 1.0],
//     [0.5, 0.5, 0.0, 1.0],
//     [-0.5, 0.5, 0.0, 1.0],
// ];
// pub const QUAD_INDICES: &[u32] = &[0, 1, 2, 2, 3, 0];

pub const TRI_POSITIONS: &[[f32; 4]] = &[
    [-1.0, -0.577, 0.0, 1.0],
    [1.0, -0.577, 0.0, 1.0],
    [0.0, 1.155, 0.0, 1.0],
];

pub const TRI_INDICES: &[u32] = &[0, 1, 2];

#[derive(Resource)]
pub struct PointCloudMesh {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub index_count: u32,
}

impl FromWorld for PointCloudMesh {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let vertex_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("pcl_octree_mesh_vertex_buffer"),
            contents: bytemuck::cast_slice(TRI_POSITIONS),
            usage: BufferUsages::VERTEX,
        });

        let index_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("pcl_octree_mesh_index_buffer"),
            contents: bytemuck::cast_slice(TRI_INDICES),
            usage: BufferUsages::INDEX,
        });

        bevy_log::info!("Buffer len: {}", vertex_buffer.size());

        Self {
            vertex_buffer,
            index_buffer,
            index_count: TRI_INDICES.len() as u32,
        }
    }
}
