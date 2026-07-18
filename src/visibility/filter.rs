use bevy::transform::components::GlobalTransform;

use crate::PointCloudNode;

use super::CameraView;

pub struct ScreenPixelRadiusFilter {
    pub min_radius: Option<f32>,
}

impl ScreenPixelRadiusFilter {
    /// Returns `true` if the node have to be filtered (hidden).
    /// `false` if it should'nt.
    #[inline]
    pub fn filter(
        &self,
        node: &PointCloudNode,
        _global_transform: &GlobalTransform,
        _camera_view: &CameraView,
        screen_pixel_radius: Option<f32>,
    ) -> bool {
        if node.depth == 0 {
            return false;
        }
        match (screen_pixel_radius, self.min_radius) {
            (_, None) | (None, _) => false,
            (Some(radius), Some(min_radius)) if radius >= min_radius => false,
            _ => true,
        }
    }
}
