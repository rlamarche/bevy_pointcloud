use std::sync::Arc;

use bevy::{
    asset::{AssetId, Handle},
    ecs::{component::Component, entity::Entity},
    platform::collections::HashMap,
    reflect::Reflect,
};

use crate::{ChildIndex, ChildrenMask, HierarchyNode, NodeId, PointCloud, PointCloudChunk};

#[derive(Clone, Debug, Component, Reflect)]
pub struct PointCloudVisibilitySettings {
    pub min_radius: Option<f32>,
    pub point_budget: Option<usize>,
}

impl Default for PointCloudVisibilitySettings {
    fn default() -> Self {
        Self {
            min_radius: Some(30.0),
            point_budget: Some(10_000_000),
        }
    }
}

/// This component stores the visible nodes for each point cloud at view level (camera) in "main
/// world".
#[derive(Component, Clone, Debug, Default)]
pub struct VisiblePointCloudEntities {
    pub entities: HashMap<Entity, VisiblePointCloudEntity>,
    pub changed_this_frame: bool,
}

impl VisiblePointCloudEntities {
    pub fn get_mut(&mut self, entity: Entity) -> &mut VisiblePointCloudEntity {
        self.entities.entry(entity).or_default()
    }

    pub fn clear_all(&mut self) {
        // Don't just nuke the hash table; we want to reuse allocations.
        for point_cloud_entities in self.entities.values_mut() {
            point_cloud_entities.asset_id = Default::default();
            point_cloud_entities.node_entities.clear();
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct VisiblePointCloudEntity {
    pub asset_id: AssetId<PointCloud>,
    pub node_entities: Vec<VisiblePointCloudNodeEntity>,
}

/// Contains useful informations about a visible node
#[derive(Clone, Default, Debug)]
pub struct VisiblePointCloudNodeEntity {
    /// the corresponding chunk entity, if available
    pub id: NodeId,
    pub chunk_id: Option<AssetId<PointCloudChunk>>,
    pub name: Arc<str>,
    pub parent_id: Option<NodeId>,
    pub depth: u32,
    pub child_index: ChildIndex,
    pub children: [usize; 8],
    pub children_mask: ChildrenMask,
    pub entity: Option<Entity>,
}

impl From<&HierarchyNode> for VisiblePointCloudNodeEntity {
    fn from(value: &HierarchyNode) -> Self {
        VisiblePointCloudNodeEntity {
            id: value.id,
            chunk_id: value.chunk.as_ref().map(Handle::id),
            name: value.name.clone(),
            parent_id: value.parent_id,
            depth: value.depth,
            child_index: value.child_index,
            children: [0_usize; 8],
            children_mask: ChildrenMask::empty(),
            entity: None,
        }
    }
}

/// This component stores the visible nodes for each octree at view level (camera) in "main world".
#[derive(Component)]
pub struct SkipPointCloudVisibility;
