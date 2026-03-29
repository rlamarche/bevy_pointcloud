use std::sync::Arc;

use crate::octree::hierarchy::HierarchyNodeStatus;
use crate::octree::loader::{LoadedHierarchyNode, OctreeLoader};
use crate::pointcloud_octree::asset::data::PointCloudNodeData;
use async_trait::async_trait;
use bevy_camera::primitives::Aabb;
use bevy_ecs::error::BevyError;
use bevy_reflect::TypePath;
use potree::asset::PotreeAsset;
use potree::asset::url::PotreeUrlAsset;
use potree::hierarchy::HierarchyAsync;
use potree::metadata::Points;
use potree::octree::node::{NodeType, OctreeNode as PotreeOctreeNode};
use potree::prelude::Hierarchy;

pub struct PotreeLoader<T> {
    pub(crate) hierarchy: Hierarchy<T>,
}

#[derive(Clone, TypePath)]
pub struct PotreeHierarchy(pub(crate) PotreeOctreeNode);

#[async_trait]
impl<T: PotreeAsset + 'static> OctreeLoader<PointCloudNodeData> for PotreeLoader<T> {
    type Source = T;
    type Hierarchy = PotreeHierarchy;
    type Error = BevyError;

    async fn from_source(
        source: Self::Source,
    ) -> Result<Self, <Self as OctreeLoader<PointCloudNodeData>>::Error> {
        let hierarchy = Hierarchy::new(source.into()).await?;

        Ok(PotreeLoader { hierarchy })
    }

    async fn load_initial_hierarchy(
        &self,
    ) -> Result<Vec<LoadedHierarchyNode<PotreeHierarchy>>, Self::Error> {
        let hierarchy = self.hierarchy.load_initial_hierarchy().await?;

        Ok(hierarchy.into_iter().map(Into::into).collect())
    }

    async fn load_hierarchy(
        &self,
        node: &LoadedHierarchyNode<PotreeHierarchy>,
    ) -> Result<Vec<LoadedHierarchyNode<PotreeHierarchy>>, Self::Error> {
        Ok(self
            .hierarchy
            .load_hierarchy(&node.data.0)
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    async fn load_node_data(
        &self,
        node: &LoadedHierarchyNode<PotreeHierarchy>,
    ) -> Result<PointCloudNodeData, Self::Error> {
        let Points { density, points } = self.hierarchy.load_points(&node.data.0).await?;

        // magic formula from Potree
        let offset = (density as f32).log2() / 2.0 - 1.5;

        // info!("Loaded {} points", points.len());

        Ok(PointCloudNodeData {
            spacing: node.data.0.spacing as f32,
            level: node.data.0.level,
            offset,
            num_points: node.data.0.num_points as usize,
            points: Arc::new(points.into_iter().map(Into::into).collect()),
        })
    }
}

impl From<PotreeOctreeNode> for LoadedHierarchyNode<PotreeHierarchy> {
    fn from(value: PotreeOctreeNode) -> Self {
        Self {
            status: match value.node_type {
                NodeType::Proxy => HierarchyNodeStatus::Proxy,
                _ => HierarchyNodeStatus::Loaded,
            },
            child_index: value.child_index,
            parent_id: value.parent,
            bounding_box: Aabb::from_min_max(
                value.bounding_box.min.as_vec3(),
                value.bounding_box.max.as_vec3(),
            ),
            data: PotreeHierarchy(value),
        }
    }
}

impl From<(&PotreeOctreeNode, Points)> for PointCloudNodeData {
    fn from((node, Points { points, density }): (&PotreeOctreeNode, Points)) -> Self {
        // magic formula from Potree
        let offset = (density as f32).log2() / 2.0 - 1.5;

        PointCloudNodeData {
            spacing: node.spacing as f32,
            level: node.level,
            offset,
            num_points: node.num_points as usize,
            points: Arc::new(points.into_iter().map(Into::into).collect()),
        }
    }
}
