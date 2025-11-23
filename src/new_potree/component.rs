use crate::new_potree::loader::PotreeHierarchy;
use crate::octree::new_asset::asset::NewOctree;
use crate::pointcloud_octree::asset::PointCloudNodeData;
use bevy_asset::{AssetId, Handle};
use bevy_ecs::prelude::*;

#[derive(Component)]
pub struct NewPotreePointCloud3d(pub Handle<NewOctree<PotreeHierarchy, PointCloudNodeData>>);

impl Into<AssetId<NewOctree<PotreeHierarchy, PointCloudNodeData>>> for &NewPotreePointCloud3d {
    fn into(self) -> AssetId<NewOctree<PotreeHierarchy, PointCloudNodeData>> {
        self.0.clone().id()
    }
}
