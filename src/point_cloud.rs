use bevy_asset::{AsAssetId, Asset, AssetId, Handle};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::component::Component;
use bevy_ecs::reflect::ReflectComponent;
use bevy_math::Vec3;
use bevy_reflect::{Reflect, std_traits::ReflectDefault};
use bevy_transform::prelude::*;
use bytemuck::{Pod, Zeroable};

pub const QUAD_POSITIONS: &[[f32; 3]] = &[
    [-0.5, -0.5, 0.0],
    [0.5, -0.5, 0.0],
    [0.5, 0.5, 0.0],
    [-0.5, 0.5, 0.0],
];
pub const QUAD_INDICES: &[u32] = &[0, 1, 2, 2, 3, 0];

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

#[derive(Component, Clone, Debug, Default, Deref, DerefMut, Reflect, PartialEq, Eq)]
#[reflect(Component, Default, Clone, PartialEq)]
#[require(Transform)]
pub struct PointCloud3d(pub Handle<PointCloud>);

impl From<PointCloud3d> for AssetId<PointCloud> {
    fn from(point_cloud: PointCloud3d) -> Self {
        point_cloud.id()
    }
}

impl From<&PointCloud3d> for AssetId<PointCloud> {
    fn from(pointcloud: &PointCloud3d) -> Self {
        pointcloud.id()
    }
}

impl AsAssetId for PointCloud3d {
    type Asset = PointCloud;

    fn as_asset_id(&self) -> AssetId<Self::Asset> {
        self.id()
    }
}
