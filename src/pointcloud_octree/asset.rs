use bevy_reflect::TypePath;
use bevy_math::prelude::*;
use crate::octree::asset::Octree;

pub type PointCloudOctree = Octree<PointCloudNodeData>;


#[derive(Default, Debug, Clone, TypePath)]
pub struct PointCloudNodeData {
    pub spacing: f32,
    pub level: u32,
    pub num_points: usize,
    pub points: Vec<PointData>,
}

#[derive(Default, Debug, Clone, TypePath)]
pub struct PointData {
    pub position: Vec3,
    pub color: Vec4,
}

