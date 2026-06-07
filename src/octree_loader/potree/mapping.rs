use bevy_ecs::error::BevyError;
use bevy_math::Vec3;
use potree::point::{PointBuffer, PointSlice};

use crate::point::RGBPoint;

impl<'a> From<PointSlice<'a>> for RGBPoint {
    fn from(val: PointSlice<'a>) -> Self {
        let position = val
            .attribute_type(potree::point::AttributeType::Position)
            .map(Vec3::from_slice)
            .unwrap_or_default();

        let color = val
            .attribute_type(potree::point::AttributeType::Rgb)
            .map(Vec3::from_slice)
            .unwrap_or_default()
            .extend(1.0);

        RGBPoint::new(position, color)
    }
}

pub fn convert_potree_points_to_points<T>(point_buffer: &PointBuffer) -> Vec<T>
where
    T: for<'a> From<PointSlice<'a>>,
{
    point_buffer.iter().map(|point| point.into()).collect()
}

pub fn try_convert_potree_points_to_points<T>(
    point_buffer: &PointBuffer,
) -> Result<Vec<T>, BevyError>
where
    T: for<'a> TryFrom<PointSlice<'a>, Error: Into<BevyError>>,
{
    point_buffer
        .iter()
        .map(|point| point.try_into().map_err(Into::into))
        .collect()
}
