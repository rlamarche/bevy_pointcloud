use crate::point_cloud::{PointCloud, PointCloudData};
use bevy_app::{App, Plugin};
use bevy_asset::io::Reader;
use bevy_asset::{AssetApp, AssetLoader, LoadContext};
use bevy_log::info;
use bevy_math::Vec3;
use bevy_reflect::erased_serde::__private::serde::{Deserialize, Serialize};
use std::io::{Cursor, Error};
use thiserror::Error;

/// Naive implementation of a las loader because it loads the las file completely in memory
pub struct LasLoaderPlugin;

impl Plugin for LasLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.register_asset_loader(LasLoader {});
    }
}

/// An error that occurs when loading a glTF file.
#[derive(Error, Debug)]
pub enum LasLoaderError {
    /// Failed to load a file.
    #[error("failed to read las: {0}")]
    LoadError(#[from] las::Error),
    /// Failed to load a file.
    #[error("failed to load file: {0}")]
    Io(#[from] Error),
}

#[derive(Default, Serialize, Deserialize)]
pub struct LasLoaderSettings {}

pub struct LasLoader {}

impl AssetLoader for LasLoader {
    type Asset = PointCloud;
    type Settings = LasLoaderSettings;
    type Error = LasLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<PointCloud, Self::Error> {
        // reader.read_to_end()
        let mut las_data = Vec::new();
        reader.read_to_end(&mut las_data).await?;

        let reader = Cursor::new(las_data);

        let mut points = Vec::new();
        let mut las_reader = las::Reader::new(reader)?;

        let mut min = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
        let mut max = Vec3::new(f32::MIN, f32::MIN, f32::MIN);

        las_reader.points().into_iter().for_each(|point| {
            let point = point.unwrap();
            let vec = Vec3::new(point.x as f32, point.y as f32, point.z as f32);

            min = min.min(vec);
            max = max.max(vec);
        });

        las_reader.seek(0).unwrap();

        for wrapped_point in las_reader.points() {
            let point = wrapped_point.unwrap();

            let delta = -min;
            // let delta = (max - min) / 2.0;

            if let Some(color) = point.color {
                points.push(PointCloudData {
                    position: Vec3::new(
                        point.x as f32 + delta.x,
                        point.z as f32 + delta.z,
                        point.y as f32 + delta.y,
                    ),
                    // < 0.0 means every points have the same size (taken from the material)
                    point_size: -1.0,
                    // color,
                    color: [
                        color.red as f32 / u16::MAX as f32,
                        color.green as f32 / u16::MAX as f32,
                        color.blue as f32 / u16::MAX as f32,
                        1.0,
                    ],
                });
            }
        }

        info!("Loaded point cloud with {} points", points.len());

        Ok(PointCloud { points })
    }
}
