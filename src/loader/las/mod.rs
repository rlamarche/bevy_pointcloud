use std::{
    fmt::Display,
    io::{Cursor, Error},
    marker::PhantomData,
    sync::Arc,
};

use bevy_app::{App, Plugin};
use bevy_asset::{io::Reader, AssetApp, AssetLoader, LoadContext};
use bevy_log::{info, warn};
use bevy_math::prelude::*;
use bevy_reflect::TypePath;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{point::Point, point_cloud::PointCloud};

/// Naive implementation of a las loader because it loads the las file completely in memory
pub struct LasLoaderPlugin<T: Point>(PhantomData<T>)
where
    T: TryFrom<las::Point>,
    <T as TryFrom<las::Point>>::Error: Display;

impl<T: Point> Default for LasLoaderPlugin<T>
where
    T: TryFrom<las::Point>,
    <T as TryFrom<las::Point>>::Error: Display,
{
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: Point> Plugin for LasLoaderPlugin<T>
where
    T: TryFrom<las::Point>,
    <T as TryFrom<las::Point>>::Error: Display,
{
    fn build(&self, app: &mut App) {
        app.register_asset_loader(LasLoader::<T>(PhantomData));
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

#[derive(TypePath)]
pub struct LasLoader<T: Point>(PhantomData<T>);

impl<T: Point> AssetLoader for LasLoader<T>
where
    T: TryFrom<las::Point>,
    <T as TryFrom<las::Point>>::Error: Display,
{
    type Asset = PointCloud<T>;
    type Settings = LasLoaderSettings;
    type Error = LasLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<PointCloud<T>, Self::Error> {
        // reader.read_to_end()
        let mut las_data = Vec::new();
        reader.read_to_end(&mut las_data).await?;

        let reader = Cursor::new(las_data);

        let mut points = Vec::new();
        let mut las_reader = las::Reader::new(reader)?;

        let mut min = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
        let mut max = Vec3::new(f32::MIN, f32::MIN, f32::MIN);

        las_reader.points().for_each(|point| {
            let point = point.unwrap();
            let vec = Vec3::new(point.x as f32, point.y as f32, point.z as f32);

            min = min.min(vec);
            max = max.max(vec);
        });

        las_reader.seek(0).unwrap();

        let mut first_error: Option<<T as TryFrom<las::Point>>::Error> = None;

        for wrapped_point in las_reader.points() {
            let point = wrapped_point.expect("error reading las point");
            // let position = Vec4::new(point.x as f32, point.z as f32, -point.y as f32, -1.0);
            match T::try_from(point) {
                Ok(point) => {
                    points.push(point);
                    // color: Vec4::new(
                    //     color.red as f32 / u16::MAX as f32,
                    //     color.green as f32 / u16::MAX as f32,
                    //     color.blue as f32 / u16::MAX as f32,
                    //     1.0,
                    // ),
                }
                Err(error) => {
                    // keep only the first error
                    if first_error.is_none() {
                        first_error = Some(error)
                    }
                }
            }
        }

        if let Some(error) = first_error {
            warn!(
                "Loaded point cloud with {} points with warning: {:#}",
                points.len(),
                error
            );
        } else {
            info!("Loaded point cloud with {} points", points.len());
        }
        Ok(PointCloud {
            points: Arc::new(points),
        })
    }

    fn extensions(&self) -> &[&str] {
        &["las", "laz"]
    }
}
