use std::io::{Cursor, Seek};

use bevy::{
    app::{App, Plugin},
    asset::{AssetApp, AssetLoader, RenderAssetUsages},
    camera::primitives::{Aabb, MeshAabb},
    log::{info, warn},
    math::Vec3,
    mesh::{Mesh, VertexAttributeValues},
    reflect::TypePath,
};
use thiserror::Error;

use crate::{
    ByteSource, ByteSourceError, ChildIndex, ChunkLoadResult, LoadedPointCloudNode, PointCloud,
    PointCloudChunk, PointCloudLoader, PointCloudNodeStatus, PointCloudTopology,
};

/// Naive implementation of a las loader because it loads the las file completely in memory
pub struct LasLoaderPlugin;

impl Plugin for LasLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.register_asset_loader(LasAssetLoader);
    }
}

/// An error that occurs when loading a glTF file.
#[derive(Error, Debug)]
pub enum LasLoaderError {
    /// Failed to load a file.
    #[error("failed to read las: {0}")]
    LoadError(#[from] las::Error),
    /// Failed to load a file.
    #[error("failed to load source: {0}")]
    ByteSource(#[from] ByteSourceError),
    /// Failed to load a file.
    #[error("failed to load file: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
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
    type Settings = ();

    async fn from_source(
        source: Self::Source,
        settings: Self::Settings,
    ) -> Result<Self, Self::Error> {
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

    async fn load_chunk(
        &self,
        &point_count: &Self::Hierarchy,
    ) -> Result<ChunkLoadResult, Self::Error> {
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

        Ok(ChunkLoadResult {
            mesh: Some(mesh),
            final_point_count: point_count,
        })
    }
}

#[derive(TypePath)]
pub struct LasAssetLoader;

impl AssetLoader for LasAssetLoader {
    type Asset = PointCloud;

    type Settings = LasLoaderSettings;

    type Error = LasLoaderError;

    async fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _settings: &Self::Settings,
        load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let reader = Cursor::new(bytes);

        let mut las_reader = las::Reader::new(reader)?;
        let point_count = las_reader.points().count();

        las_reader.seek(0).unwrap();

        let mesh = load_points_as_mesh(point_count, &mut las_reader);
        let vertex_buffer_size = mesh.get_vertex_buffer_size();

        let aabb = mesh.compute_aabb();

        let mesh_handle = load_context.add_labeled_asset("mesh", mesh);

        let chunk_handle = load_context.add_labeled_asset(
            "chunk",
            PointCloudChunk {
                depth: 0,
                mesh_handle: Some(mesh_handle),
                aabb: aabb.clone(),
                vertex_buffer_size,
            },
        );

        Ok(PointCloud {
            aabb,
            topology: PointCloudTopology::Flat(chunk_handle),
        })
    }

    fn extensions(&self) -> &[&str] {
        &["las", "laz"]
    }
}

fn load_points_as_mesh(point_count: usize, las_reader: &mut las::Reader) -> Mesh {
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

    mesh
}
