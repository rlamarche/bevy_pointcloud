// mod density;

use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
};

use async_lock::RwLock;
use bevy::{
    asset::RenderAssetUsages,
    camera::primitives::Aabb,
    math::DVec3,
    mesh::{Mesh, VertexAttributeValues},
    platform::collections::HashMap,
    prelude::Deref,
};
use copc_streaming::{CopcError, CopcStreamingReader, HierarchyEntry, VoxelKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ByteSource, ByteSourceError, ChildIndex, LoadedPointCloudNode, PointCloudLoader,
    PointCloudNodeStatus,
};

/// An error that occurs when loading a glTF file.
#[derive(Error, Debug)]
pub enum CopcLoaderError {
    /// Failed to load a file.
    #[error("error reading copc source: {0}")]
    ByteSource(#[from] ByteSourceError),

    #[error("error reading copc: {0}")]
    CopcError(#[from] CopcError),

    #[error("root node is missing in the copc hierarchy")]
    RootMissing,

    #[error("an invalid hierarchy has been encountered: {0}")]
    InvalidHierarchy(String),
}

impl From<ByteSourceError> for CopcError {
    fn from(value: ByteSourceError) -> Self {
        match value {
            ByteSourceError::Io(error) => CopcError::Io(error),
            ByteSourceError::ByteSource(error) => CopcError::ByteSource(error),
        }
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct CopcLoaderSettings {
    pub filter_classification: Option<HashSet<u32>>,
}

pub struct CopcLoader<S: ByteSource> {
    reader: Arc<RwLock<CopcStreamingReader<CopcByteSource<S>>>>,
}

#[derive(Clone, Debug, Deref)]
pub struct CopcHierarchy(HierarchyEntry);

pub struct CopcByteSource<S: ByteSource>(S);

impl<S: ByteSource> From<S> for CopcByteSource<S> {
    fn from(value: S) -> Self {
        Self(value)
    }
}

impl<S: ByteSource> copc_streaming::ByteSource for CopcByteSource<S> {
    async fn read_range(&self, offset: u64, length: u64) -> Result<Vec<u8>, CopcError> {
        self.0.read_range(offset, length).await.map_err(Into::into)
    }

    async fn size(&self) -> Result<Option<u64>, CopcError> {
        self.0.size().await.map_err(Into::into)
    }
}

impl<S: ByteSource> PointCloudLoader for CopcLoader<S> {
    type Source = S;
    type Hierarchy = CopcHierarchy;
    type Error = CopcLoaderError;
    type Settings = CopcLoaderSettings;

    async fn from_source(source: Self::Source) -> Result<Self, Self::Error> {
        let copc_source: CopcByteSource<S> = source.into();
        let reader = CopcStreamingReader::open(copc_source).await?;

        Ok(Self {
            reader: Arc::new(RwLock::new(reader)),
        })
    }

    async fn load_initial_hierarchy(
        &self,
    ) -> Result<Vec<LoadedPointCloudNode<Self::Hierarchy>>, Self::Error> {
        let mut reader = self.reader.write().await;

        // TODO: read the hiearchy in a lazy way (implementing load_sub_hierarchy)
        reader.load_all_hierarchy().await?;

        let copc_info = reader.copc_info();
        // info!("COPC INFO: {:#?}", copc_info);

        // let las_header = reader.header().las_header();
        // info!("LAS HEADER: {:#?}", las_header);

        let aabb = copc_info.root_bounds();

        let mut initial_hierarchy = Vec::new();

        // contains a mapping between voxel key and index in output vec
        let mut parent_indexes = HashMap::<VoxelKey, usize>::new();

        // stack of the nodes to process
        let mut stack = VecDeque::<(&HierarchyEntry, Option<VoxelKey>)>::new();
        let root_hierarchy = reader
            .get(&VoxelKey {
                level: 0,
                x: 0,
                y: 0,
                z: 0,
            })
            .ok_or(CopcLoaderError::RootMissing)?;

        // initialize the stack with the root node
        stack.push_back((root_hierarchy, None));

        // iterate recursively in hierarchy tree to gather hierarchy nodes
        while let Some((hierarchy_entry, parent_key)) = stack.pop_front() {
            // add root node to index
            parent_indexes.insert(hierarchy_entry.key, initial_hierarchy.len());

            // add root node to hierarchy vec
            initial_hierarchy.push(LoadedPointCloudNode {
                status: PointCloudNodeStatus::Loaded,
                child_index: hierarchy_entry.key.into(),
                // retrieve the parent id in the map
                parent_index: parent_key
                    .map(|key| {
                        parent_indexes.get(&key).copied().ok_or_else(|| {
                            CopcLoaderError::InvalidHierarchy(format!(
                                "Voxel key {:?} is missing in parent indexes",
                                key
                            ))
                        })
                    })
                    .transpose()?,
                aabb: Some(copc_aabb_to_aabb(hierarchy_entry.key.bounds(&aabb))),
                data: CopcHierarchy(hierarchy_entry.clone()),
                point_count: hierarchy_entry.point_count as usize,
            });

            // load children
            let children = reader.children(&hierarchy_entry.key);

            // append children to stack
            for child in children {
                stack.push_back((child, Some(hierarchy_entry.key)));
            }
        }

        Ok(initial_hierarchy)
    }

    async fn load_chunk(&self, node: &Self::Hierarchy) -> Result<Mesh, Self::Error> {
        let key = node.key;
        let reader = self.reader.read().await;

        let las_header = reader.header().las_header();

        let has_color = las_header.point_format().has_color;
        // TODO use settings
        let has_normal = false;

        let chunk = reader.fetch_chunk(&key).await?;

        // let spacing = reader.copc_info().spacing;
        // let aabb = node.key.bounds(&reader.copc_info().root_bounds());
        let points = reader.read_points(&chunk)?;

        drop(reader);

        // let level = node.key.level as f64;

        // let density = compute_density(&points, &aabb);

        // let spacing = (spacing / level.exp2()) as f32;

        // // magic formula from Potree
        // let offset = (density as f32).log2() / 2.0 - 1.5;

        let point_count = node.point_count as usize;

        // allocate data
        let mut positions: Vec<[f32; 3]> = Vec::with_capacity(point_count);

        // will be allocated if a color is found
        let mut maybe_colors: Option<Vec<[f32; 4]>> = match has_color {
            true => Some(Vec::with_capacity(point_count)),
            false => None,
        };

        let mut maybe_normals: Option<Vec<[f32; 3]>> = match has_normal {
            true => Some(Vec::with_capacity(point_count)),
            false => None,
        };

        // TODO load more attributes
        for point in points {
            positions.push([point.x as f32, point.y as f32, point.z as f32]);

            if let Some(colors) = maybe_colors.as_mut() {
                if let Some(color) = point.color {
                    colors.push([
                        (color.red as f64 / 65535.0) as f32,
                        (color.green as f64 / 65535.0) as f32,
                        (color.blue as f64 / 65535.0) as f32,
                        1.0,
                    ]);
                } else {
                    // insert an empty color to prevent holes
                    colors.push([0.0, 0.0, 0.0, 1.0]);
                }
            }
            if let Some(normals) = maybe_normals.as_mut() {
                // always z up normal (testing)
                normals.push([0.0, 0.0, 1.0]);
            }
        }

        let mut mesh = Mesh::new(
            bevy::mesh::PrimitiveTopology::PointList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            VertexAttributeValues::Float32x3(positions),
        );

        if let Some(colors) = maybe_colors {
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_COLOR,
                VertexAttributeValues::Float32x4(colors),
            );
        }

        if let Some(normals) = maybe_normals {
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_NORMAL,
                VertexAttributeValues::Float32x3(normals),
            );
        }

        Ok(mesh)
    }
}

impl From<VoxelKey> for ChildIndex {
    fn from(value: VoxelKey) -> Self {
        let x = value.x & 1;
        let y = value.y & 1;
        let z = value.z & 1;

        // Safe to unwrap
        ((x * 4 + y * 2 + z) as u8).try_into().unwrap()
    }
}

fn copc_aabb_to_aabb(value: copc_streaming::Aabb) -> Aabb {
    let min = DVec3::from_array(value.min);
    let max = DVec3::from_array(value.max);

    Aabb::from_min_max(min.as_vec3(), max.as_vec3())
}
