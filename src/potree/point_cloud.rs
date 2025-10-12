use bevy_asset::AsAssetId;
use crate::potree::asset::PotreePointCloud;
use bevy_asset::prelude::*;
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use bevy_reflect::prelude::*;
use bevy_transform::prelude::*;

#[derive(Component)]
pub struct PotreeMainCamera;

#[derive(Component, Clone, Debug, Default, Deref, DerefMut, Reflect, PartialEq, Eq)]
#[reflect(Component, Default, Clone, PartialEq)]
#[require(Transform)]
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
