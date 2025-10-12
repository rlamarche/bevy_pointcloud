use crate::pointcloud_octree::asset::PointCloudOctree;
use crate::potree::asset::PotreePointCloud;
use async_lock::RwLock;
use bevy_asset::io::Reader;
use bevy_asset::{AssetLoader, LoadContext};
use bevy_camera::primitives::Aabb;
use bevy_log::prelude::*;
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

        let point_cloud =
            potree::point_cloud::PotreePointCloud::from_url(&asset_path, ResourceLoader::new())
                .await?;

        info!("Potree Point Cloud loaded");

        // Get root node
        let root = point_cloud.octree().root();
        // Load its points
        let points = point_cloud.load_points_for_node(root).await?;
        // Convert its bounding box
        let bounding_box = Aabb::from_min_max(
            root.bounding_box.min.as_vec3(),
            root.bounding_box.max.as_vec3(),
        );

        let octree = PointCloudOctree::new(bounding_box, (root, points).into());

        let octree = load_context.add_labeled_asset(
            "octree".to_string(),
            octree,
        );

        Ok(PotreePointCloud {
            wrapped: Arc::new(RwLock::new(point_cloud)),
            octree,
        })
    }
}
