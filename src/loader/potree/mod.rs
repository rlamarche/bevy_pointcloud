mod asset;

use std::sync::Arc;

use bevy::{
    asset::RenderAssetUsages,
    camera::primitives::Aabb,
    log::warn,
    math::DVec3,
    mesh::{Mesh, VertexAttributeValues},
    platform::collections::HashSet,
    prelude::Deref,
};
use potree::{
    hierarchy::{HierarchyAsync, PotreeHierarchyError},
    octree::node::{NodeType, OctreeNode as PotreeOctreeNode},
    point::AttributeType,
    prelude::Hierarchy,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use asset::*;

use crate::{
    BuilderNodeId, ByteSource, ByteSourceError, ChildIndex, ChunkLoadResult, InsertNodeParams,
    OctreeError, OctreeHierarchyBuilder, OctreeLoader, OctreeMetadata, PointCloudNodeStatus,
};

/// An error that occurs when loading Potree point clouds.
#[derive(Error, Debug)]
pub enum PotreeLoaderError {
    #[error("error reading byte source: {0}")]
    ByteSource(#[from] ByteSourceError),

    #[error("root node is missing in the potree hierarchy")]
    RootMissing,

    #[error("invalid hierarchy: {0}")]
    InvalidHierarchy(String),

    #[error("metadata loading error: {0}")]
    Metadata(String),

    #[error("potree internal error: {0}")]
    Potree(#[from] PotreeHierarchyError),

    #[error("octree topology error: {0}")]
    Octree(#[from] OctreeError),
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

pub struct PotreeLoader<S: ByteSource> {
    hierarchy: Arc<Hierarchy<PotreeAssetSource<S>>>,
    settings: PotreeLoaderSettings,
}

#[derive(Clone, Debug, Deref)]
pub struct PotreeHierarchy(pub PotreeOctreeNode);

impl<S: ByteSource + Send + Sync + 'static> OctreeLoader for PotreeLoader<S> {
    type Source = PotreeAssetSource<S>;
    type Hierarchy = PotreeHierarchy;
    type Error = PotreeLoaderError;
    type Settings = PotreeLoaderSettings;

    async fn from_source(
        source: Self::Source,
        settings: Self::Settings,
    ) -> Result<Self, Self::Error> {
        let hierarchy = Hierarchy::load(source).await?;

        Ok(Self {
            hierarchy: Arc::new(hierarchy),
            settings,
        })
    }

    async fn load_metadata(&self) -> Result<OctreeMetadata, Self::Error> {
        let metadata = self.hierarchy.metadata();

        Ok(OctreeMetadata {
            point_count: Some(metadata.points),
            aabb: Some(Aabb::from_min_max(
                DVec3::from_array(metadata.bounding_box.min).as_vec3(),
                DVec3::from_array(metadata.bounding_box.max).as_vec3(),
            )),
            spacing: Some(metadata.spacing),
        })
    }

    async fn load_initial_hierarchy(
        &self,
        builder: &mut OctreeHierarchyBuilder<Self::Hierarchy>,
    ) -> Result<(), Self::Error> {
        let raw_nodes = self.hierarchy.load_initial_hierarchy().await?;
        build_hierarchy::<S>(builder, raw_nodes)?;

        Ok(())
    }

    async fn load_sub_hierarchy(
        &self,
        node: &Self::Hierarchy,
        builder: &mut OctreeHierarchyBuilder<Self::Hierarchy>,
    ) -> Result<(), Self::Error> {
        let raw_nodes = self.hierarchy.load_hierarchy(node).await?;
        build_hierarchy::<S>(builder, raw_nodes)?;

        Ok(())
    }

    async fn load_chunk(&self, node: &Self::Hierarchy) -> Result<ChunkLoadResult, Self::Error> {
        let points = self.hierarchy.load_points(&node.0).await?;
        // magic formula from Potree
        let offset = (points.density as f32).log2() / 2.0 - 1.5;

        // Extract point slice from the raw buffer provided by potree crate
        let raw_point_count = points.buffer.count;

        if raw_point_count == 0 {
            return Ok(ChunkLoadResult {
                mesh: None,
                offset: None,
                final_point_count: 0,
            });
        }

        let position_attribute = points
            .buffer
            .layout
            .iter()
            .find(|attribute_info| attribute_info.r#type.eq(&AttributeType::Position));
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

        let normal_x_attribute = points
            .buffer
            .layout
            .iter()
            .find(|attribute_info| attribute_info.r#type.eq(&AttributeType::NormalX));
        let normal_y_attribute = points
            .buffer
            .layout
            .iter()
            .find(|attribute_info| attribute_info.r#type.eq(&AttributeType::NormalY));
        let normal_z_attribute = points
            .buffer
            .layout
            .iter()
            .find(|attribute_info| attribute_info.r#type.eq(&AttributeType::NormalZ));

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
                offset: None,
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

        let mut maybe_normals: Option<Vec<[f32; 3]>> = if normal_attribute.is_some()
            || (normal_x_attribute.is_some()
                && normal_y_attribute.is_some()
                && normal_z_attribute.is_some())
        {
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
                if let Some(classification) = point.attribute(classification_attribute) {
                    let class_val = classification[0];

                    if !self.settings.filter_classification.filter(class_val as u8) {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            if let Some(position_attribute) = position_attribute
                && let Some(pos) = point.attribute(position_attribute)
            {
                positions.push([pos[0], pos[1], pos[2]]);
            } else {
                positions.push([0.0, 0.0, 0.0]);
            }

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
                } else if let (
                    Some(normal_x_attribute),
                    Some(normal_y_attribute),
                    Some(normal_z_attribute),
                ) = (normal_x_attribute, normal_y_attribute, normal_z_attribute)
                    && let (Some(normal_x), Some(normal_y), Some(normal_z)) = (
                        point.attribute(normal_x_attribute),
                        point.attribute(normal_y_attribute),
                        point.attribute(normal_z_attribute),
                    )
                {
                    normals.push([normal_x[0], normal_y[0], normal_z[0]]);
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
            offset: Some(offset),
            final_point_count,
        })
    }
}

fn build_hierarchy<S: ByteSource + Send + Sync + 'static>(
    builder: &mut OctreeHierarchyBuilder<PotreeHierarchy>,
    mut raw_nodes: Vec<PotreeOctreeNode>,
) -> Result<(), <PotreeLoader<S> as OctreeLoader>::Error> {
    let (children, roots) = build_hierarchy_children(&raw_nodes);
    let Some(&root_idx) = roots.first() else {
        return Err(PotreeLoaderError::InvalidHierarchy(
            "Loaded octree hierarchy is empty or missing a root node".to_string(),
        ));
    };
    if roots.len() > 1 {
        warn!(
            "Loaded octree hierarchy contains {} root nodes; using the first one",
            roots.len()
        );
    }
    let mut inserted_nodes: Vec<Option<BuilderNodeId>> = vec![None; raw_nodes.len()];
    let mut stack = vec![(root_idx, None)];
    while let Some((idx, parent_id)) = stack.pop() {
        if inserted_nodes[idx].is_some() {
            continue;
        }

        let node = std::mem::take(&mut raw_nodes[idx]);
        let aabb = Aabb::from_min_max(node.bounding_box.min, node.bounding_box.max);
        let node_id = if let Some(parent_id) = parent_id {
            builder.insert_child(
                parent_id,
                ChildIndex::try_from(node.child_index)
                    .map_err(|e| PotreeLoaderError::InvalidHierarchy(e.to_string()))?,
                InsertNodeParams {
                    status: match node.node_type {
                        NodeType::Proxy => PointCloudNodeStatus::Proxy,
                        _ => PointCloudNodeStatus::Loaded,
                    },
                    point_count: node.num_points as usize,
                    aabb: Some(aabb),
                },
                PotreeHierarchy(node),
            )?
        } else {
            builder.insert_root(
                InsertNodeParams {
                    status: match node.node_type {
                        NodeType::Proxy => PointCloudNodeStatus::Proxy,
                        _ => PointCloudNodeStatus::Loaded,
                    },
                    point_count: node.num_points as usize,
                    aabb: Some(aabb),
                },
                PotreeHierarchy(node),
            )?
        };
        inserted_nodes[idx] = Some(node_id);

        for &child_idx in children[idx].iter().rev() {
            stack.push((child_idx, Some(node_id)));
        }
    }

    Ok(())
}

/// Build a child adjacency list and collect root indices for hierarchy vectors.
fn build_hierarchy_children(nodes: &[PotreeOctreeNode]) -> (Vec<Vec<usize>>, Vec<usize>) {
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    let mut roots = Vec::new();

    for (idx, node) in nodes.iter().enumerate() {
        if let Some(parent) = node.parent {
            if parent < nodes.len() {
                children[parent].push(idx);
            } else {
                warn!(
                    "Hierarchy node {} references parent {} but only {} nodes exist",
                    idx,
                    parent,
                    nodes.len()
                );
            }
        } else {
            roots.push(idx);
        }
    }

    (children, roots)
}
