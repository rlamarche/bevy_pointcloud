use crate::pointcloud_octree::asset::PointCloudOctree;
use crate::pointcloud_octree::component::PointCloudOctree3d;
use crate::potree::asset::PotreePointCloud;
use bevy_asset::AsAssetId;
use bevy_asset::prelude::*;
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::lifecycle::HookContext;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemState;
use bevy_ecs::world::DeferredWorld;
use bevy_log::prelude::*;
use bevy_reflect::prelude::*;
use bevy_transform::prelude::*;

#[derive(Component)]
pub struct PotreeMainCamera;

#[derive(Component, Clone, Debug, Default, Deref, DerefMut, Reflect, PartialEq, Eq)]
#[reflect(Component, Default, Clone, PartialEq)]
#[require(Transform)]
#[component(on_add = add_pointcloud_octree)]
pub struct PotreePointCloud3d {
    pub handle: Handle<PotreePointCloud>,
}

impl AsAssetId for PotreePointCloud3d {
    type Asset = PotreePointCloud;

    fn as_asset_id(&self) -> AssetId<Self::Asset> {
        self.id()
    }
}

impl From<PotreePointCloud3d> for AssetId<PotreePointCloud> {
    fn from(value: PotreePointCloud3d) -> Self {
        value.id()
    }
}

impl From<&PotreePointCloud3d> for AssetId<PotreePointCloud> {
    fn from(value: &PotreePointCloud3d) -> Self {
        value.id()
    }
}

/// Add point cloud octree asset & component for rendering
pub fn add_pointcloud_octree(
    mut world: DeferredWorld<'_>,
    HookContext { entity, .. }: HookContext,
)
{
    let mut point_cloud_octrees = world
        .get_resource_mut::<Assets<PointCloudOctree>>()
        .expect("PointCloudOctree resource missing");

    let point_cloud_octree_handle = point_cloud_octrees.add(PointCloudOctree::new());

    world
        .commands()
        .entity(entity)
        .insert(PointCloudOctree3d(point_cloud_octree_handle));
}

