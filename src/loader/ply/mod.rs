use std::io::{BufReader, Cursor, Error};

use bevy_app::{App, Plugin};
use bevy_asset::{io::Reader, AssetApp, AssetLoader, LoadContext};
use bevy_log;
use bevy_math::Vec3;
use bevy_reflect::TypePath;
use ply_rs::{
    parser::Parser,
    ply::{Property, PropertyAccess},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::point_cloud::{PointCloud, PointCloudData};

#[derive(TypePath)]
pub struct PlyLoaderPlugin;

impl Plugin for PlyLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.register_asset_loader(PlyLoader {});
    }
}

impl PropertyAccess for PointCloudData {
    fn new() -> Self {
        PointCloudData {
            position: Vec3::ZERO,
            point_size: 1.0,
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }

    fn set_property(&mut self, key: String, property: Property) {
        match (key.as_ref(), property) {
            ("x", Property::Float(v)) => self.position[0] = v,
            ("y", Property::Float(v)) => self.position[1] = v,
            ("z", Property::Float(v)) => self.position[2] = v,
            ("red", Property::UChar(v)) => self.color[0] = v as f32 / u8::MAX as f32,
            ("green", Property::UChar(v)) => self.color[1] = v as f32 / u8::MAX as f32,
            ("blue", Property::UChar(v)) => self.color[2] = v as f32 / u8::MAX as f32,
            (_, v) => {
                // bevy_log::warn!("Unhandled PLY property: {} {:?}", key, v);
            }
        }
    }
}

/// An error that occurs when loading a glTF file.
#[derive(Error, Debug)]
pub enum PlyLoaderError {
    /// Failed to load a file.
    #[error("failed to read las: {0}")]
    LoadError(#[from] las::Error),
    /// Failed to load a file.
    #[error("failed to load file: {0}")]
    Io(#[from] Error),
}

#[derive(Default, Serialize, Deserialize)]
pub struct PlyLoaderSettings {}

#[derive(TypePath)]
pub struct PlyLoader {}

impl AssetLoader for PlyLoader {
    type Asset = PointCloud;
    type Settings = PlyLoaderSettings;
    type Error = PlyLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<PointCloud, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let cursor = Cursor::new(bytes);
        let mut f = BufReader::new(cursor);

        let parser = Parser::<PointCloudData>::new();
        let header = parser.read_header(&mut f)?;

        let mut cloud = Vec::new();

        let required_properties = ["x", "y", "z", "red", "green", "blue"];
        let mut required_property_count = required_properties.len();

        for (_key, element) in &header.elements {
            if element.name == "vertex" {
                for (key, _prop) in &element.properties {
                    required_property_count -= required_properties.contains(&key.as_str()) as usize;
                }

                if required_property_count > 0 {
                    return Err(PlyLoaderError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "missing required properties",
                    )));
                }

                cloud = parser.read_payload_for_element(&mut f, element, &header)?;
            }
        }
        bevy_log::info!("Loaded point cloud with {} points", cloud.len());

        Ok(PointCloud { points: cloud })
    }

    fn extensions(&self) -> &[&str] {
        &["ply"]
    }
}
