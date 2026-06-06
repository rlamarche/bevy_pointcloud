use bevy_asset::{AsAssetId, Asset, AssetId, Handle};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::component::Component;
use bevy_reflect::TypePath;
use bevy_transform::prelude::*;

use crate::point::Point;

pub const QUAD_POSITIONS: &[[f32; 3]] = &[
    [-0.5, -0.5, 0.0],
    [0.5, -0.5, 0.0],
    [0.5, 0.5, 0.0],
    [-0.5, 0.5, 0.0],
];
pub const QUAD_INDICES: &[u32] = &[0, 1, 2, 2, 3, 0];

#[derive(Debug, Clone, Asset, TypePath)]
pub struct PointCloud<T: Point> {
    pub points: Vec<T>,
}

#[derive(Component, Clone, Debug, Default, Deref, DerefMut, TypePath, PartialEq, Eq)]
#[require(Transform)]
pub struct PointCloud3d<T: Point>(pub Handle<PointCloud<T>>);

impl<T: Point> From<PointCloud3d<T>> for AssetId<PointCloud<T>> {
    fn from(point_cloud: PointCloud3d<T>) -> Self {
        point_cloud.id()
    }
}

impl<T: Point> From<&PointCloud3d<T>> for AssetId<PointCloud<T>> {
    fn from(pointcloud: &PointCloud3d<T>) -> Self {
        pointcloud.id()
    }
}

impl<T: Point> AsAssetId for PointCloud3d<T> {
    type Asset = PointCloud<T>;

    fn as_asset_id(&self) -> AssetId<Self::Asset> {
        self.id()
    }
}
