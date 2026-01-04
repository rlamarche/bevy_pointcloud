use super::CameraView;
use crate::octree::node::{NodeData, OctreeNode};
use bevy_transform::prelude::*;

pub trait OctreeHierarchyFilter<T: NodeData>: Send + Sync {
    type Settings: Send + Sync;

    fn new(settings: Self::Settings) -> Self;

    fn filter(
        &self,
        node: &OctreeNode<T>,
        global_transform: &GlobalTransform,
        camera_view: &CameraView,
        screen_pixel_radius: Option<f32>,
    ) -> bool;
}

pub struct ScreenPixelRadiusFilter {
    min_radius: f32,
}

impl<T: NodeData> OctreeHierarchyFilter<T> for ScreenPixelRadiusFilter {
    type Settings = f32;

    fn new(min_radius: Self::Settings) -> Self {
        Self { min_radius }
    }

    fn filter(
        &self,
        _node: &OctreeNode<T>,
        _global_transform: &GlobalTransform,
        _camera_view: &CameraView,
        screen_pixel_radius: Option<f32>,
    ) -> bool {
        if let Some(radius) = screen_pixel_radius
            && radius < self.min_radius
        {
            false
        } else {
            true
        }
    }
}
