use bevy::transform::components::GlobalTransform;

use crate::PointCloudNode;

use super::CameraView;

pub struct ScreenPixelRadiusFilter {
    pub min_radius: Option<f32>,
}

impl ScreenPixelRadiusFilter {
    pub fn filter(
        &self,
        _node: &PointCloudNode,
        _global_transform: &GlobalTransform,
        _camera_view: &CameraView,
        screen_pixel_radius: Option<f32>,
    ) -> bool {
        match (screen_pixel_radius, self.min_radius) {
            (_, None) | (None, _) => true,
            (Some(radius), Some(min_radius)) if radius >= min_radius => true,
            _ => false,
        }
    }
}
