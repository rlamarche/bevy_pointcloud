use bevy_math::prelude::*;
use bevy_reflect::TypePath;
use bytemuck::{Pod, Zeroable};

use super::{GpuPoint, Point};

#[derive(Clone, Copy, Debug, TypePath, Pod, Zeroable)]
#[repr(C)]
pub struct RGBPoint {
    pub position: Vec3,
    pub _padding: f32,
    pub color: Vec4,
}

impl Point for RGBPoint {
    fn position(&self) -> &Vec3 {
        &self.position
    }
}

impl From<&RGBPoint> for RGBPoint {
    fn from(value: &RGBPoint) -> Self {
        *value
    }
}

impl GpuPoint for RGBPoint {}

#[cfg(feature = "las")]
impl From<las::Point> for RGBPoint {
    fn from(point: las::Point) -> Self {
        let color = point.color.unwrap_or_default();
        Self {
            position: Vec3::new(point.x as f32, point.z as f32, -point.y as f32),
            _padding: 0.0,
            color: Vec4::new(
                color.red as f32 / u16::MAX as f32,
                color.green as f32 / u16::MAX as f32,
                color.blue as f32 / u16::MAX as f32,
                1.0,
            ),
        }
    }
}
