use crate::pointcloud_octree::asset::PointCloudOctree;
use crate::potree::asset::PotreePointCloud;
use async_lock::RwLock;
use bevy_asset::io::Reader;
use bevy_asset::{AssetLoader, LoadContext};
use bevy_camera::primitives::Aabb;
use bevy_log::prelude::*;
use potree::hierarchy::{Hierarchy, LoadPotreePointCloudError};
use potree::resource::ResourceLoader;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

#[derive(Default, Serialize, Deserialize)]
pub struct PotreeLoaderSettings {}

#[derive(Error, Debug)]
pub enum PotreeLoaderError {
    /// Failed to load a file.
    #[error("failed to load potree point cloud: {0}")]
    Potree(#[from] potree::prelude::LoadPotreePointCloudError),

    #[error("failed to load potree points: {0}")]
    LoadPoints(#[from] potree::prelude::LoadPointsError),

    #[error("invalid path")]
    InvalidPath,
}

pub struct PotreeLoader {}

impl AssetLoader for PotreeLoader {
    type Asset = PotreePointCloud;
    type Settings = PotreeLoaderSettings;
    type Error = PotreeLoaderError;

    async fn load(
        &self,
        _reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut path = load_context
            .path()
            .to_str()
            .ok_or(PotreeLoaderError::InvalidPath)?;

        if path.ends_with("/metadata.json") {
            path = path.strip_suffix("/metadata.json").unwrap();
        }

        let asset_path = if path.starts_with("/") {
            format!("file://{}", path).to_string()
        } else {
            // transform to relative asset path
            format!("assets/{}", path).to_string()
        };

        info!("Loading Potree Point Cloud from path {}", asset_path);

        let point_cloud = Hierarchy::from_url(&asset_path, ResourceLoader::new()).await?;

        info!("Potree Point Cloud loaded");

        Ok(PotreePointCloud {
            hierarchy: Arc::new(RwLock::new(point_cloud)),
        })
    }
}
