use bevy_asset::prelude::*;
use bevy_math::prelude::*;
use bevy_reflect::prelude::*;
use bytemuck::{Pod, Zeroable};

#[derive(Debug, Clone, Asset, Reflect)]
pub struct PointCloud {
    pub points: Vec<PointCloudData>,
}

#[derive(Debug, Clone, Copy, Reflect, Pod, Zeroable)]
#[repr(C)]
pub struct PointCloudData {
    pub position: Vec3,
    pub point_size: f32,
    pub color: [f32; 4],
}
