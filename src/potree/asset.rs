use async_lock::RwLock;
use bevy_asset::prelude::*;
use bevy_reflect::prelude::*;
use std::sync::Arc;

#[derive(Debug, Clone, Asset, TypePath)]
pub struct PotreePointCloud {
    pub hierarchy: Arc<RwLock<potree::hierarchy::Hierarchy>>,
}
