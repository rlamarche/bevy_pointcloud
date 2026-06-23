use std::sync::Arc;

use bevy_reflect::TypePath;

use crate::{octree::node::NodeData, point::Point};

#[derive(Default, Debug, Clone, TypePath)]
pub struct PointCloudNodeData<T: Point> {
    pub spacing: f32,
    pub level: u32,
    /// offset applied to point size
    pub offset: f32,
    pub point_count: usize,
    pub points: Arc<Vec<T>>,
}

impl<T: Point> NodeData for PointCloudNodeData<T> {
    fn size(&self) -> usize {
        self.point_count * size_of::<T>()
    }

    fn instance_count(&self) -> usize {
        self.point_count
    }
}
