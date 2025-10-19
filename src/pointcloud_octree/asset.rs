use crate::octree::asset::Octree;
use bevy_math::prelude::*;
use bevy_reflect::TypePath;
use bytemuck::{Pod, Zeroable};

pub type PointCloudOctree = Octree<PointCloudNodeData>;

#[derive(Default, Debug, Clone, TypePath)]
pub struct PointCloudNodeData {
    pub spacing: f32,
    pub level: u32,
    pub num_points: usize,
    pub points: Vec<PointData>,
}

#[derive(Default, Debug, Clone, Copy, TypePath, Pod, Zeroable)]
#[repr(C)]
pub struct PointData {
    // position + padding
    pub position: Vec4,
    pub color: Vec4,
}
