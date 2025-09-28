use std::sync::Arc;
use async_lock::RwLock;
use bevy_asset::prelude::*;
use bevy_reflect::prelude::*;



#[derive(Debug, Clone, Asset, TypePath)]
pub struct PotreePointCloud {
    pub wrapped: Arc<RwLock<potree::point_cloud::PotreePointCloud>>,
}
