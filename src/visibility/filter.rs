use bevy::transform::components::GlobalTransform;

use crate::HierarchyNode;

use super::CameraView;

pub struct ScreenPixelRadiusFilter {
    pub min_radius: Option<f32>,
}

impl ScreenPixelRadiusFilter {
    pub fn filter(
        &self,
        _node: &HierarchyNode,
        _global_transform: &GlobalTransform,
        _camera_view: &CameraView,
        screen_pixel_radius: Option<f32>,
    ) -> bool {
        if let (Some(radius), Some(min_radius)) = (screen_pixel_radius, self.min_radius)
            && radius >= min_radius
        {
            true
        } else {
            false
        }
    }
}
