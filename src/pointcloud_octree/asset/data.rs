use std::sync::Arc;

use bevy_reflect::TypePath;
use bevy_render::render_resource::AsBindGroup;

use crate::{octree::node::NodeData, point::Point};

#[derive(Default, Debug, Clone, TypePath, AsBindGroup)]
pub struct PointCloudNodeData<T: Clone + TypePath> {
    #[uniform(0)]
    pub spacing: f32,
    #[uniform(1)]
    pub level: u32,
    /// offset applied to point size
    #[uniform(2)]
    pub offset: f32,
    pub num_points: usize,
    pub points: Arc<Vec<T>>,
}

// #[derive(Default, Debug, Clone, Copy, Pod, Zeroable, TypePath)]
// #[repr(C)]
// pub struct PointData {
//     // position + padding
//     pub position: Vec4,
//     pub color: Vec4,
// }

impl<T: Point> NodeData for PointCloudNodeData<T> {
    fn size(&self) -> usize {
        self.num_points * size_of::<T>()
    }

    fn instance_count(&self) -> usize {
        self.num_points
    }
}
