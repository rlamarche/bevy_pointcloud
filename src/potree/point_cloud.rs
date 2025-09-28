use crate::potree::asset::PotreePointCloud;
use bevy_asset::prelude::*;
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use bevy_reflect::prelude::*;
use bevy_transform::prelude::*;

#[derive(Component)]
pub struct PotreeMainCamera;

#[derive(Component, Clone, Debug, Default, Reflect, PartialEq, Eq)]
#[require(Transform)]
#[reflect(Component, Default, Clone, PartialEq)]
pub struct PotreePointCloud3d {
    pub handle: Handle<PotreePointCloud>,
}
