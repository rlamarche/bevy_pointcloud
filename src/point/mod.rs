mod rgb;

use bevy_math::prelude::*;
use bevy_mesh::VertexBufferLayout;
use bevy_reflect::TypePath;
use bevy_render::render_resource::{VertexAttribute, VertexStepMode};
use bytemuck::{Pod, Zeroable};
pub use rgb::*;

pub trait Point: Clone + Sync + Send + TypePath {
    fn position(&self) -> &Vec3;
}

pub trait GpuPoint: Point + Pod + Zeroable {
    fn vertex_attributes() -> Vec<VertexAttribute>;

    fn vertex_buffer_layout() -> VertexBufferLayout {
        VertexBufferLayout {
            array_stride: size_of::<Self>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: Self::vertex_attributes(),
        }
    }
}
