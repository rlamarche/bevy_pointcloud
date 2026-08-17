mod asset;

use std::sync::Arc;

use bevy::{
    asset::RenderAssetUsages,
    camera::primitives::Aabb,
    mesh::{Mesh, VertexAttributeValues},
    platform::collections::HashSet,
    prelude::Deref,
};
use potree::{
    asset::PotreeAsset,
    hierarchy::HierarchyAsync,
    octree::node::{NodeType, OctreeNode as PotreeOctreeNode},
    point::AttributeType,
    prelude::Hierarchy,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use asset::*;

use crate::{
    ByteSourceError, ChildIndex, ChunkLoadResult, LoadedPointCloudNode, PointCloudLoader,
    PointCloudNodeStatus,
};

/// An error that occurs when loading Potree point clouds.
#[derive(Error, Debug)]
pub enum PotreeLoaderError {
    #[error("error reading byte source: {0}")]
    ByteSource(#[from] ByteSourceError),

    #[error("potree internal error: {0}")]
    Potree(String),

    #[error("root node is missing in the potree hierarchy")]
    RootMissing,

    #[error("invalid hierarchy: {0}")]
    InvalidHierarchy(String),
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct PotreeLoaderSettings {
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
    pub fn filter(&self, classification: u8) -> bool {
        match self {
            FilterClassification::None => true,
            FilterClassification::Include(set) => set.contains(&classification),
            FilterClassification::Exclude(set) => !set.contains(&classification),
        }
    }
}

pub struct PotreeLoader<T: PotreeAsset> {
    hierarchy: Arc<Hierarchy<T>>,
    settings: PotreeLoaderSettings,
}

#[derive(Clone, Debug, Deref)]
pub struct PotreeHierarchy(pub PotreeOctreeNode);

impl<T: PotreeAsset + Send + Sync + 'static> PointCloudLoader for PotreeLoader<T> {
    type Source = T;
    type Hierarchy = PotreeHierarchy;
    type Error = PotreeLoaderError;
    type Settings = PotreeLoaderSettings;

    async fn from_source(
        source: Self::Source,
        settings: Self::Settings,
    ) -> Result<Self, Self::Error> {
        let hierarchy = Hierarchy::load(source)
            .await
            .map_err(|e| PotreeLoaderError::Potree(e.to_string()))?;

        Ok(Self {
            hierarchy: Arc::new(hierarchy),
            settings,
        })
    }

    async fn load_initial_hierarchy(
        &self,
    ) -> Result<Vec<LoadedPointCloudNode<Self::Hierarchy>>, Self::Error> {
        let raw_nodes = self
            .hierarchy
            .load_initial_hierarchy()
            .await
            .map_err(|e| PotreeLoaderError::Potree(e.to_string()))?;

        let nodes = raw_nodes
            .into_iter()
            .map(|node| {
                Ok(LoadedPointCloudNode {
                    status: match node.node_type {
                        NodeType::Proxy => PointCloudNodeStatus::Proxy,
                        _ => PointCloudNodeStatus::Loaded,
                    },
                    child_index: ChildIndex::try_from(node.child_index)
                        .map_err(|e| PotreeLoaderError::InvalidHierarchy(e.to_string()))?,
                    parent_index: node.parent,
                    aabb: Some(Aabb::from_min_max(
                        node.bounding_box.min,
                        node.bounding_box.max,
                    )),
                    point_count: node.num_points as usize,
                    data: PotreeHierarchy(node),
                })
            })
            .collect::<Result<Vec<_>, Self::Error>>()?;

        Ok(nodes)
    }

    async fn load_chunk(&self, node: &Self::Hierarchy) -> Result<ChunkLoadResult, Self::Error> {
        let points = self
            .hierarchy
            .load_points(&node.0)
            .await
            .map_err(|e| PotreeLoaderError::Potree(e.to_string()))?;

        // Extract point slice from the raw buffer provided by potree crate
        let raw_point_count = points.buffer.count;

        if raw_point_count == 0 {
            return Ok(ChunkLoadResult {
                mesh: None,
                final_point_count: 0,
            });
        }

        let color_attribute = points
            .buffer
            .layout
            .iter()
            .find(|attribute_info| attribute_info.r#type.eq(&AttributeType::Rgb));
        let normal_attribute = points
            .buffer
            .layout
            .iter()
            .find(|attribute_info| attribute_info.r#type.eq(&AttributeType::Normal));
        let classification_attribute = points
            .buffer
            .layout
            .iter()
            .find(|attribute_info| attribute_info.r#type.eq(&AttributeType::Classification));

        // Calculate initial filtered count for allocation optimization
        let mut target_point_count = raw_point_count;
        if !matches!(
            self.settings.filter_classification,
            FilterClassification::None
        ) && let Some(classification_attribute) = classification_attribute
        {
            let classifications = points
                .buffer
                .attribute_slice(&classification_attribute.name)
                .expect("classification attribute presence is checked above");
            target_point_count = (0..raw_point_count)
                .filter(|&i| {
                    let class_val = classifications.get(i)[0];
                    self.settings.filter_classification.filter(class_val as u8)
                })
                .count();
        }

        if target_point_count == 0 {
            return Ok(ChunkLoadResult {
                mesh: None,
                final_point_count: 0,
            });
        }

        // Allocate vertex attributes
        let mut positions: Vec<[f32; 3]> = Vec::with_capacity(target_point_count);

        let mut maybe_colors: Option<Vec<[f32; 4]>> = if color_attribute.is_some() {
            Some(Vec::with_capacity(target_point_count))
        } else {
            None
        };

        let mut maybe_normals: Option<Vec<[f32; 3]>> = if normal_attribute.is_some() {
            Some(Vec::with_capacity(target_point_count))
        } else {
            None
        };

        // Populate attribute buffers
        for i in 0..raw_point_count {
            let Some(point) = points.buffer.get(i) else {
                // skip any missing point
                continue;
            };

            if let Some(classification_attribute) = classification_attribute {
                if let Some(class_val) = point.attribute(classification_attribute) {
                    let class_val = class_val[0];

                    if !self.settings.filter_classification.filter(class_val as u8) {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            let Some(pos) = point.attribute_type(AttributeType::Position) else {
                // skip points without positions
                continue;
            };
            positions.push([pos[0], pos[1], pos[2]]);

            if let Some(colors) = maybe_colors.as_mut() {
                if let Some(color_attribute) = color_attribute
                    && let Some(color) = point.attribute(color_attribute)
                {
                    colors.push([color[0], color[1], color[2], 1.0]);
                } else {
                    // if color is missing, push an empty color
                    colors.push([0.0, 0.0, 0.0, 1.0]);
                };
            }

            if let Some(normals) = maybe_normals.as_mut() {
                if let Some(normal_attribute) = normal_attribute
                    && let Some(normal) = point.attribute(normal_attribute)
                {
                    normals.push([normal[0], normal[1], normal[2]]);
                } else {
                    // if color is missing, push an empty color
                    normals.push([0.0, 0.0, 0.0]);
                };
            }
        }

        let final_point_count = positions.len();

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
            final_point_count,
        })
    }
}
