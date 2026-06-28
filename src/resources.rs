use bevy::{
    asset::AssetId,
    ecs::{entity::Entity, resource::Resource},
    platform::collections::HashMap,
    prelude::{Deref, DerefMut},
};

use crate::{PointCloud, PointCloudChunk};

#[derive(Resource, Debug, Default, Clone, Deref, DerefMut)]
pub struct PointCloudInstances(HashMap<AssetId<PointCloud>, PointCloudEntities>);

#[derive(Default, Debug, Clone, Deref, DerefMut)]
pub struct PointCloudEntities(HashMap<Entity, PointCloudChunks>);

#[derive(Default, Debug, Clone, Deref, DerefMut)]
pub struct PointCloudChunks(HashMap<AssetId<PointCloudChunk>, Entity>);
