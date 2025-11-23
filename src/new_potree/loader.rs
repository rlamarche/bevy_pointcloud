use crate::octree::new_asset::hierarchy::{
    HierarchyNode, HierarchyNodeData, HierarchyNodeStatus, HierarchyOctreeNode,
};
use crate::octree::new_asset::loader::OctreeLoader;
use async_trait::async_trait;
use bevy_camera::primitives::Aabb;
use bevy_ecs::error::BevyError;
use bevy_reflect::TypePath;
use potree::octree::node::{FlatOctreeNode, NodeType};
use potree::prelude::FlatHierarchy;
use potree::resource::ResourceLoader;

#[derive(TypePath)]
pub struct PotreeLoader {
    pub(crate) hierarchy: FlatHierarchy,
}

#[derive(Clone, TypePath)]
pub struct PotreeHierarchy(pub(crate) FlatOctreeNode);

impl HierarchyNodeData for PotreeHierarchy {}

#[async_trait]
impl OctreeLoader<PotreeHierarchy> for PotreeLoader {
    type Error = BevyError;

    async fn from_url(url: &str) -> Result<Self, Self::Error> {
        let hierarchy = FlatHierarchy::from_url(url, ResourceLoader::new()).await?;

        Ok(PotreeLoader { hierarchy })
    }

    async fn load_initial_hierarchy(
        &self,
    ) -> Result<Vec<HierarchyNode<PotreeHierarchy>>, Self::Error> {
        let hierarchy = self.hierarchy.load_initial_hierarchy().await?;

        Ok(hierarchy.into_iter().map(Into::into).collect())
    }

    async fn load_hierarchy(
        &self,
        node: &HierarchyOctreeNode<PotreeHierarchy>,
    ) -> Result<Vec<HierarchyNode<PotreeHierarchy>>, Self::Error> {
        Ok(self
            .hierarchy
            .load_hierarchy(&node.data.0)
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }
}

impl From<FlatOctreeNode> for HierarchyNode<PotreeHierarchy> {
    fn from(value: FlatOctreeNode) -> Self {
        HierarchyNode {
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
