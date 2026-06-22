use bevy_math::prelude::*;
use bevy_mesh::VertexFormat;
use bevy_reflect::TypePath;
use bevy_render::render_resource::VertexAttribute;
use bytemuck::{Pod, Zeroable};

use super::{GpuPoint, Point};
use crate::point_cloud::{PointCloudGpuMapper, PointCloud};

#[derive(Default, Clone, Copy, Debug, TypePath, Pod, Zeroable)]
#[repr(C)]
pub struct RGBPoint {
    pub position: Vec3,
    _padding: f32,
    pub color: Vec4,
}

impl RGBPoint {
    pub fn new(position: Vec3, color: Vec4) -> Self {
        Self {
            position,
            color,
            ..Default::default()
        }
    }
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

impl GpuPoint for RGBPoint {
    fn vertex_attributes() -> Vec<bevy_render::render_resource::VertexAttribute> {
        vec![
            // Point position
            VertexAttribute {
                format: VertexFormat::Float32x4,
                offset: 0,
                shader_location: 1,
            },
            // Point color
            VertexAttribute {
                format: VertexFormat::Float32x4,
                offset: VertexFormat::Float32x4.size(),
                shader_location: 2,
            },
        ]
    }
}

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

impl PointCloudGpuMapper for RGBPoint {
    type Point = RGBPoint;

    type GpuPoint = RGBPoint;

    fn convert(point_cloud: PointCloud<Self::Point>) -> Vec<Self::GpuPoint> {
        point_cloud.points
    }
}
