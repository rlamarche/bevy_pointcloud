use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_mesh::VertexBufferLayout;
use bevy_render::render_resource::{Buffer, BufferInitDescriptor, BufferUsages, RawBufferVec, VertexAttribute, VertexFormat, VertexStepMode};
use bevy_render::renderer::{RenderDevice, RenderQueue};
use bytemuck::{Pod, Zeroable};

static VERTICES: [Vertex; 4] = [
    Vertex::new(vec3(-0.5, -0.5, 0.0)),
    Vertex::new(vec3(0.5, -0.5, 0.0)),
    Vertex::new(vec3(0.5, 0.5, 0.0)),
    Vertex::new(vec3(-0.5, 0.5, 0.0)),
];

pub const QUAD_POSITIONS: &[[f32; 4]] = &[
    [-0.5, -0.5, 0.0, 1.0],
    [0.5, -0.5, 0.0, 1.0],
    [0.5, 0.5, 0.0, 1.0],
    [-0.5, 0.5, 0.0, 1.0],
];
pub const QUAD_INDICES: &[u32] = &[0, 1, 2, 2, 3, 0];

#[derive(Resource)]
pub struct PointCloudMesh {
    /// The vertices for the single triangle.
    ///
    /// This is a [`RawBufferVec`] because that's the simplest and fastest type
    /// of GPU buffer, and [`Vertex`] objects are simple.
    pub vertices: RawBufferVec<Vertex>,

    /// The indices of the single triangle.
    ///
    /// As above, this is a [`RawBufferVec`] because `u32` values have trivial
    /// size and alignment.
    pub indices: RawBufferVec<u32>,

    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub index_count: u32,
}

impl FromWorld for PointCloudMesh {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let render_queue = world.resource::<RenderQueue>();


        // Create the vertex and index buffers.
        let mut vbo = RawBufferVec::new(BufferUsages::VERTEX);
        let mut ibo = RawBufferVec::new(BufferUsages::INDEX);

        for vertex in &VERTICES {
            vbo.push(*vertex);
        }
        for index in QUAD_INDICES {
            ibo.push(*index);
        }

        // These two lines are required in order to trigger the upload to GPU.
        vbo.write_buffer(render_device, render_queue);
        ibo.write_buffer(render_device, render_queue);

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

        bevy_log::info!("Buffer len: {}", vertex_buffer.size());

        Self {
            vertices: vbo,
            indices: ibo,
            vertex_buffer,
            index_buffer,
            index_count: QUAD_INDICES.len() as u32,
        }
    }
}

/// The CPU-side structure that describes a single vertex of the triangle.
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Vertex {
    /// The 3D position of the triangle vertex.
    position: Vec3,
    /// Padding.
    pad0: f32,
}

impl Vertex {
    /// Creates a new vertex structure.
    const fn new(position: Vec3) -> Vertex {
        Vertex {
            position,
            pad0: 0.0,
        }
    }
}
