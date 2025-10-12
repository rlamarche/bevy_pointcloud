use crate::pointcloud_octree::asset::{PointCloudNodeData, PointData};
use bevy_math::prelude::*;
use potree::octree::node::OctreeNode;
use potree::prelude::{OctreeNodeSnapshot, PointData as PotreePointData};

impl From<&OctreeNodeSnapshot> for PointCloudNodeData {
    fn from(value: &OctreeNodeSnapshot) -> Self {
        PointCloudNodeData {
            spacing: value.spacing as f32,
            level: value.level,
            num_points: value.num_points as usize,
            points: Vec::default(),
        }
    }
}

impl From<&OctreeNode> for PointCloudNodeData {
    fn from(value: &OctreeNode) -> Self {
        PointCloudNodeData {
            spacing: value.spacing as f32,
            level: value.level,
            num_points: value.num_points as usize,
            points: Vec::default(),
        }
    }
}

impl From<(&OctreeNode, Vec<PotreePointData>)> for PointCloudNodeData {
    fn from((node, points): (&OctreeNode, Vec<PotreePointData>)) -> Self {
        PointCloudNodeData {
            spacing: node.spacing as f32,
            level: node.level,
            num_points: node.num_points as usize,
            points: points.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<PotreePointData> for PointData {
    fn from(value: PotreePointData) -> Self {
        PointData {
            position: value.position.as_vec3(),
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
            position: value.position.as_vec3(),
            color: Vec4::new(
                value.color.x as f32 / 256.0,
                value.color.y as f32 / 256.0,
                value.color.z as f32 / 256.0,
                1.0,
            ),
        }
    }
}
