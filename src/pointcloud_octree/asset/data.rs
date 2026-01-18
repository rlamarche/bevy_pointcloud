use bevy_math::prelude::*;
use bevy_reflect::TypePath;
use bevy_render::render_resource::AsBindGroup;
use bytemuck::{Pod, Zeroable};
use crate::octree::node::NodeData;

#[derive(Default, Debug, Clone, TypePath, AsBindGroup)]
pub struct PointCloudNodeData {
    #[uniform(0)]
    pub spacing: f32,
    #[uniform(1)]
    pub level: u32,
    /// offset applied to point size
    #[uniform(2)]
    pub offset: f32,
    pub num_points: usize,
    pub points: Vec<PointData>,
}

#[derive(Default, Debug, Clone, Copy, Pod, Zeroable, TypePath)]
#[repr(C)]
pub struct PointData {
    // position + padding
    pub position: Vec4,
    pub color: Vec4,
}

impl NodeData for PointCloudNodeData {
    fn size(&self) -> usize {
        self.num_points * size_of::<PointData>()
    }
}