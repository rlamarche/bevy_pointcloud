use std::io::Cursor;

use bevy::{
    app::{App, Plugin},
    asset::RenderAssetUsages,
    camera::primitives::Aabb,
    log::warn,
    math::Vec3,
    mesh::{Mesh, VertexAttributeValues},
    reflect::TypePath,
};
use thiserror::Error;

use crate::{
    ByteSource, ByteSourceError, ChildIndex, LoadedPointCloudNode, PointCloudLoader,
    PointCloudNodeStatus,
};

/// Naive implementation of a las loader because it loads the las file completely in memory
pub struct LasLoaderPlugin;

impl Plugin for LasLoaderPlugin {
    fn build(&self, _: &mut App) {
        // app.register_asset_loader(LasLoader);
    }
}

/// An error that occurs when loading a glTF file.
#[derive(Error, Debug)]
pub enum LasLoaderError {
    /// Failed to load a file.
    #[error("failed to read las: {0}")]
    LoadError(#[from] las::Error),
    /// Failed to load a file.
    #[error("failed to load las file: {0}")]
    ByteSource(#[from] ByteSourceError),
}

#[derive(Default)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct LasLoaderSettings {}

#[derive(TypePath)]
pub struct LasLoader<S> {
    source: S,
}

impl<S: ByteSource> From<S> for LasLoader<S> {
    fn from(source: S) -> Self {
        Self { source }
    }
}

impl<S: ByteSource> PointCloudLoader for LasLoader<S> {
    type Source = S;
    type Hierarchy = usize; // just the number of points for faster allocations
    type Error = LasLoaderError;

    async fn from_source(source: Self::Source) -> Result<Self, Self::Error> {
        Ok(Self::from(source))
    }

    async fn load_initial_hierarchy(
        &self,
    ) -> Result<Vec<LoadedPointCloudNode<Self::Hierarchy>>, Self::Error> {
        // TODO find an async las impl that doesn't need to own the data
        let data = self.source.read_to_end(0).await?;
        let cursor = Cursor::new(data);
        let mut las_reader = las::Reader::new(cursor)?;

        let mut min = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
        let mut max = Vec3::new(f32::MIN, f32::MIN, f32::MIN);
        let mut point_count = 0;

        las_reader.points().for_each(|point| {
            let point = point.unwrap();
            let vec = Vec3::new(point.x as f32, point.y as f32, point.z as f32);

            min = min.min(vec);
            max = max.max(vec);
            point_count += 1;
        });

        let aabb = Aabb::from_min_max(min, max);

        Ok(vec![LoadedPointCloudNode {
            status: PointCloudNodeStatus::Loaded,
            child_index: ChildIndex::ROOT,
            parent_index: None,
            aabb: Some(aabb),
            data: point_count,
            point_count,
        }])
    }

    async fn load_chunk(&self, &point_count: &Self::Hierarchy) -> Result<Mesh, Self::Error> {
        let data = self.source.read_to_end(0).await?;
        let cursor = Cursor::new(data);
        let mut las_reader = las::Reader::new(cursor)?;

        let mut positions = Vec::with_capacity(point_count);
        let mut colors = Vec::with_capacity(point_count);

        for point in las_reader.points() {
            let point = match point {
                Ok(point) => point,
                Err(e) => {
                    warn!("An error occured while parsing a point: {:#}", e);
                    continue;
                }
            };

            positions.push([point.x as f32, point.y as f32, point.z as f32]);
            if let Some(color) = &point.color {
                colors.push([
                    color.red as f32 / u16::MAX as f32,
                    color.green as f32 / u16::MAX as f32,
                    color.blue as f32 / u16::MAX as f32,
                    1.0,
                ]);
            } else {
                colors.push([0.0, 0.0, 0.0, 0.0]);
            }
        }

        let mesh = Mesh::new(
            bevy::mesh::PrimitiveTopology::PointList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            VertexAttributeValues::Float32x3(positions),
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_COLOR,
            VertexAttributeValues::Float32x4(colors),
        );

        Ok(mesh)
    }
}
