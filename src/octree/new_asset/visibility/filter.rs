use bevy_reflect::TypePath;
use crate::octree::new_asset::hierarchy::{
    HierarchyNodeData,
};
use bevy_transform::prelude::*;
use crate::octree::new_asset::node::{NodeData, OctreeNode};
use super::CameraView;

pub trait OctreeHierarchyFilter<H, T>: Send + Sync
where
    H: HierarchyNodeData,
    T: NodeData,
{
    type Settings: Send + Sync;

    fn new(settings: Self::Settings) -> Self;

    fn filter(
        &self,
        node: &OctreeNode<H, T>,
        global_transform: &GlobalTransform,
        camera_view: &CameraView,
        screen_pixel_radius: Option<f32>,
    ) -> bool;
}

pub struct ScreenPixelRadiusFilter {
    min_radius: f32,
}

impl<H, T> OctreeHierarchyFilter<H, T> for ScreenPixelRadiusFilter
where
    H: HierarchyNodeData,
    T: NodeData,
{
    type Settings = f32;

    fn new(min_radius: Self::Settings) -> Self {
        Self { min_radius }
    }

    fn filter(
        &self,
        _node: &OctreeNode<H, T>,
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
