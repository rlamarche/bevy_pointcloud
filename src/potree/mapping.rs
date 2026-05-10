use bevy_math::prelude::*;
use potree::prelude::PointData as PotreePointData;

use crate::pointcloud_octree::asset::data::PointData;

impl From<PotreePointData> for PointData {
    fn from(value: PotreePointData) -> Self {
        PointData {
            position: value.position.as_vec3().extend(1.0),
            color: Vec4::new(
                value.color.x as f32 / 256.0,
                value.color.y as f32 / 256.0,
                value.color.z as f32 / 256.0,
                1.0,
            ),
        }
    }
}

impl From<&PotreePointData> for PointData {
    fn from(value: &PotreePointData) -> Self {
        PointData {
            position: value.position.as_vec3().extend(1.0),
            color: Vec4::new(
                value.color.x as f32 / 256.0,
                value.color.y as f32 / 256.0,
                value.color.z as f32 / 256.0,
                1.0,
            ),
        }
    }
}
