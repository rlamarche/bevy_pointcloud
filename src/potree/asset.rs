use crate::pointcloud_octree::asset::PointCloudOctree;
use async_lock::RwLock;
use bevy_asset::prelude::*;
use bevy_reflect::prelude::*;
use std::sync::Arc;

#[derive(Debug, Clone, Asset, TypePath)]
pub struct PotreePointCloud {
    pub wrapped: Arc<RwLock<potree::point_cloud::PotreePointCloud>>,
    pub octree: Handle<PointCloudOctree>,
}
