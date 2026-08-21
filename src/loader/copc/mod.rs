// mod density;

use std::{collections::VecDeque, sync::Arc};

use async_lock::RwLock;
use bevy::{
    asset::RenderAssetUsages,
    camera::primitives::Aabb,
    math::DVec3,
    mesh::{Mesh, VertexAttributeValues},
    platform::collections::{HashMap, HashSet},
    prelude::Deref,
};
use copc_streaming::{CopcError, CopcStreamingReader, HierarchyEntry, VoxelKey};
use las::point::Classification;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    BuilderNodeId, ByteSource, ByteSourceError, ChildIndex, ChunkLoadResult, InsertNodeParams,
    OctreeError, OctreeHierarchyBuilder, OctreeLoader, OctreeMetadata, PointCloudNodeStatus,
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

    #[error("octree topology error: {0}")]
    Octree(#[from] OctreeError),
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
    pub filter_classification: FilterClassification,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub enum FilterClassification {
    #[default]
    None,
    Include(HashSet<u8>),
    Exclude(HashSet<u8>),
}

impl FilterClassification {
    pub fn filter(&self, classification: &Classification) -> bool {
        match self {
            FilterClassification::None => true,
            FilterClassification::Include(hash_set) => hash_set.contains(&classification.as_u8()),
            FilterClassification::Exclude(hash_set) => !hash_set.contains(&classification.as_u8()),
        }
    }
}

pub struct CopcLoader<S: ByteSource> {
    reader: Arc<RwLock<CopcStreamingReader<CopcByteSource<S>>>>,
    settings: CopcLoaderSettings,
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

impl<S: ByteSource> OctreeLoader for CopcLoader<S> {
    type Source = S;
    type Hierarchy = CopcHierarchy;
    type Error = CopcLoaderError;
    type Settings = CopcLoaderSettings;

    async fn from_source(
        source: Self::Source,
        settings: Self::Settings,
    ) -> Result<Self, Self::Error> {
        let copc_source: CopcByteSource<S> = source.into();
        let reader = CopcStreamingReader::open(copc_source).await?;

        Ok(Self {
            reader: Arc::new(RwLock::new(reader)),
            settings,
        })
    }

    async fn load_metadata(&self) -> Result<OctreeMetadata, Self::Error> {
        let reader = self.reader.read().await;
        let copc_info = reader.copc_info();
        let aabb = copc_info.root_bounds();

        Ok(OctreeMetadata {
            point_count: None,
            aabb: Some(Aabb::from_min_max(
                DVec3::from_array(aabb.min).as_vec3(),
                DVec3::from_array(aabb.max).as_vec3(),
            )),
            spacing: Some(copc_info.spacing as f32),
        })
    }

    async fn load_initial_hierarchy(
        &self,
        builder: &mut OctreeHierarchyBuilder<Self::Hierarchy>,
    ) -> Result<(), Self::Error> {
        let mut reader = self.reader.write().await;

        // TODO: read the hiearchy in a lazy way (implementing load_sub_hierarchy)
        reader.load_all_hierarchy().await?;

        let copc_info = reader.copc_info();
        // info!("COPC INFO: {:#?}", copc_info);

        // let las_header = reader.header().las_header();
        // info!("LAS HEADER: {:#?}", las_header);

        let aabb = copc_info.root_bounds();

        // contains a mapping between voxel key and index in output vec
        let mut parent_indexes = HashMap::<VoxelKey, BuilderNodeId>::new();

        // stack of the nodes to process
        let mut stack = VecDeque::<(&HierarchyEntry, Option<BuilderNodeId>)>::new();
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
        while let Some((hierarchy_entry, parent_id)) = stack.pop_front() {
            let inserted_id = if let Some(parent_id) = parent_id {
                builder.insert_child(
                    parent_id,
                    hierarchy_entry.key.into(),
                    InsertNodeParams {
                        status: PointCloudNodeStatus::Loaded,
                        point_count: hierarchy_entry.point_count as usize,
                        aabb: Some(copc_aabb_to_aabb(hierarchy_entry.key.bounds(&aabb))),
                    },
                    CopcHierarchy(hierarchy_entry.clone()),
                )?
            } else {
                builder.insert_root(
                    InsertNodeParams {
                        status: PointCloudNodeStatus::Loaded,
                        point_count: hierarchy_entry.point_count as usize,
                        aabb: Some(copc_aabb_to_aabb(hierarchy_entry.key.bounds(&aabb))),
                    },
                    CopcHierarchy(hierarchy_entry.clone()),
                )?
            };
            parent_indexes.insert(hierarchy_entry.key, inserted_id);

            // load children
            let children = reader.children(&hierarchy_entry.key);

            // append children to stack
            for child in children {
                stack.push_back((child, Some(inserted_id)));
            }
        }

        Ok(())
    }

    async fn load_chunk(&self, node: &Self::Hierarchy) -> Result<ChunkLoadResult, Self::Error> {
        let key = node.key;
        let reader = self.reader.read().await;

        let las_header = reader.header().las_header();

        let has_color = las_header.point_format().has_color;
        // TODO use settings
        let has_normal = false;

        // let spacing = reader.copc_info().spacing;
        // let aabb = node.key.bounds(&reader.copc_info().root_bounds());
        let points = reader.fetch_points(&key).await?;

        drop(reader);

        // let level = node.key.level as f64;

        // let density = compute_density(&points, &aabb);

        // let spacing = (spacing / level.exp2()) as f32;

        // // magic formula from Potree
        // let offset = (density as f32).log2() / 2.0 - 1.5;

        let mut point_count = node.point_count as usize;

        // update point count based on the filter
        if !matches!(
            self.settings.filter_classification,
            FilterClassification::None
        ) {
            point_count = points
                .iter()
                .filter(|point| {
                    self.settings
                        .filter_classification
                        .filter(&point.classification)
                })
                .count();
        }

        // early exit if there is no points
        if point_count == 0 {
            return Ok(ChunkLoadResult {
                mesh: None,
                offset: None,
                final_point_count: 0,
            });
        }

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
            // filter points based on classification
            if !self
                .settings
                .filter_classification
                .filter(&point.classification)
            {
                continue;
            }

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

        Ok(ChunkLoadResult {
            mesh: Some(mesh),
            offset: None, // TODO compute density then offset
            final_point_count: point_count,
        })
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

/// Extension trait providing access to the raw ASPRS numerical code.
pub trait ClassificationExt {
    /// Returns the underlying ASPRS code as a `u8`.
    fn as_u8(&self) -> u8;
}

impl ClassificationExt for Classification {
    fn as_u8(&self) -> u8 {
        match *self {
            Classification::CreatedNeverClassified => 0,
            Classification::Unclassified => 1,
            Classification::Ground => 2,
            Classification::LowVegetation => 3,
            Classification::MediumVegetation => 4,
            Classification::HighVegetation => 5,
            Classification::Building => 6,
            Classification::LowPoint => 7,
            Classification::ModelKeyPoint => 8,
            Classification::Water => 9,
            Classification::Rail => 10,
            Classification::RoadSurface => 11,
            // Code 12 (Overlap) is intentionally excluded from the enum by design.
            Classification::WireGuard => 13,
            Classification::WireConductor => 14,
            Classification::TransmissionTower => 15,
            Classification::WireStructureConnector => 16,
            Classification::BridgeDeck => 17,
            Classification::HighNoise => 18,
            Classification::Reserved(code) | Classification::UserDefinable(code) => code,
        }
    }
}
