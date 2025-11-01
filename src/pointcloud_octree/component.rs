use bevy_asset::{AsAssetId, AssetId, Handle};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use bevy_reflect::{Reflect, std_traits::ReflectDefault};
use crate::pointcloud_octree::asset::PointCloudOctree;

#[derive(Component, Clone, Debug, Default, Deref, DerefMut, Reflect, PartialEq, Eq)]
#[reflect(Component, Default, Clone, PartialEq, Debug)]
pub struct PointCloudOctree3d(pub Handle<PointCloudOctree>);

impl AsAssetId for PointCloudOctree3d {
    type Asset = PointCloudOctree;

    fn as_asset_id(&self) -> AssetId<Self::Asset> {
        self.id()
    }
}

impl From<PointCloudOctree3d> for AssetId<PointCloudOctree> {
    fn from(point_cloud: PointCloudOctree3d) -> Self {
        point_cloud.id()
    }
}

impl From<&PointCloudOctree3d> for AssetId<PointCloudOctree> {
    fn from(pointcloud: &PointCloudOctree3d) -> Self {
        pointcloud.id()
    }
}
