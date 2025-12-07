use crate::octree::new_asset::hierarchy::{
    HierarchyNodeData, HierarchyOctreeNode,
};
use bevy_transform::prelude::*;
use super::CameraView;

pub trait OctreeHierarchyFilter<H>: Send + Sync
where
    H: HierarchyNodeData,
{
    type Settings: Send + Sync;

    fn new(settings: Self::Settings) -> Self;

    fn filter(
        &self,
        node: &HierarchyOctreeNode<H>,
        global_transform: &GlobalTransform,
        camera_view: &CameraView,
        screen_pixel_radius: Option<f32>,
    ) -> bool;
}

pub struct ScreenPixelRadiusFilter {
    min_radius: f32,
}

impl<H> OctreeHierarchyFilter<H> for ScreenPixelRadiusFilter
where
    H: HierarchyNodeData,
{
    type Settings = f32;

    fn new(min_radius: Self::Settings) -> Self {
        Self { min_radius }
    }

    fn filter(
        &self,
        _node: &HierarchyOctreeNode<H>,
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
