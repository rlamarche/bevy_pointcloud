use std::sync::Arc;

use bevy::{
    asset::{AssetId, Handle},
    ecs::{
        component::Component,
        entity::{Entity, EntityHashMap},
        reflect::ReflectComponent,
    },
    reflect::{std_traits::ReflectDefault, Reflect},
};

use crate::{ChildIndex, ChildrenMask, NodeId, PointCloud, PointCloudChunk, PointCloudNode};

#[derive(Clone, Debug, Component, Reflect)]
pub struct PointCloudVisibilitySettings {
    pub min_radius: Option<f32>,
    pub point_budget: Option<usize>,
    pub max_depth: Option<u32>,
}

impl Default for PointCloudVisibilitySettings {
    fn default() -> Self {
        Self {
            min_radius: Some(30.0),
            point_budget: Some(10_000_000),
            max_depth: None,
        }
    }
}

/// This component stores the visible nodes for each point cloud octree at view level (camera) in
/// "main world".
#[derive(Component, Clone, Debug, Default, Reflect)]
#[reflect(Component, Default, Clone)]
pub struct VisiblePointCloudOctreeEntities {
    #[reflect(ignore, clone)]
    pub entities: EntityHashMap<VisiblePointCloudOctreeEntity>,
    pub changed_this_frame: bool,
}

impl VisiblePointCloudOctreeEntities {
    pub fn get_mut(&mut self, entity: Entity) -> &mut VisiblePointCloudOctreeEntity {
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

#[derive(Component, Clone, Debug, Default, Reflect)]
#[reflect(Component, Default, Clone)]
pub struct VisiblePointCloudFlatEntities {
    #[reflect(ignore, clone)]
    pub entities: EntityHashMap<AssetId<PointCloud>>,
    pub changed_this_frame: bool,
}

impl VisiblePointCloudFlatEntities {
    pub fn clear(&mut self) {
        self.entities.clear();
    }
}

#[derive(Clone, Debug, Default)]
pub struct VisiblePointCloudOctreeEntity {
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
    /// the larger, the more visible is this chunk (so prioritized)
    pub weight: f32,
}

impl VisiblePointCloudNodeEntity {
    pub fn from_with_weight(value: &PointCloudNode, weight: f32) -> Self {
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
            weight,
        }
    }
}
// impl From<&PointCloudNode> for VisiblePointCloudNodeEntity {
//     fn from(value: &PointCloudNode) -> Self {
//         VisiblePointCloudNodeEntity {
//             id: value.id,
//             chunk_id: value.chunk.as_ref().map(Handle::id),
//             name: value.name.clone(),
//             parent_id: value.parent_id,
//             depth: value.depth,
//             child_index: value.child_index,
//             children: [0_usize; 8],
//             children_mask: ChildrenMask::empty(),
//             entity: None,
//         }
//     }
// }

/// This component stores the visible nodes for each octree at view level (camera) in "main world".
#[derive(Component)]
pub struct SkipPointCloudVisibility;
