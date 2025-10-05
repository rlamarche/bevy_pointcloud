use crate::loader::las::LasLoaderError;
use crate::point_cloud::{PointCloud, PointCloudData};
use crate::potree::asset::PotreePointCloud;
use bevy_app::{App, Plugin};
use bevy_asset::io::Reader;
use bevy_asset::{AssetApp, AssetLoader, LoadContext};
use bevy_log::prelude::*;
use potree::resource::ResourceLoader;
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Error};
use std::sync::Arc;
use async_lock::RwLock;
use thiserror::Error;

#[derive(Default, Serialize, Deserialize)]
pub struct PotreeLoaderSettings {}

#[derive(Error, Debug)]
pub enum PotreeLoaderError {
    /// Failed to load a file.
    #[error("failed to load potree point cloud: {0}")]
    Potree(#[from] potree::prelude::LoadPotreePointCloudError),

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


        let point_cloud =
            potree::point_cloud::PotreePointCloud::from_url(&asset_path, ResourceLoader::new()).await?;

        info!("Potree Point Cloud loaded");

        Ok(PotreePointCloud {
            wrapped: Arc::new(RwLock::new(point_cloud)),
        })
    }
}
