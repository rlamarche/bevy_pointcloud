use std::sync::Arc;

use bevy::{
    asset::AssetId,
    ecs::{component::Component, entity::Entity},
    platform::collections::HashMap,
    render::sync_world::MainEntity,
};

use crate::{ChildIndex, ChildrenMask, NodeId, PointCloud, PointCloudChunk};

/// This component stores the visible nodes for each point cloud at view level (camera) in "render
/// world".
#[derive(Debug, Component, Default, Clone)]
pub struct RenderVisiblePointCloudEntities {
    pub entities: HashMap<(Entity, MainEntity), RenderVisiblePointCloudEntity>,
    pub changed_this_frame: bool,
}

impl RenderVisiblePointCloudEntities {
    pub fn get_mut(
        &mut self,
        (entity, main_entity): (Entity, MainEntity),
    ) -> &mut RenderVisiblePointCloudEntity {
        self.entities.entry((entity, main_entity)).or_default()
    }

    pub fn clear_all(&mut self) {
        // Don't just nuke the hash table; we want to reuse allocations.
        for render_point_cloud_entities in self.entities.values_mut() {
            render_point_cloud_entities.asset_id = Default::default();
            render_point_cloud_entities.chunk_entities.clear();
        }
        self.changed_this_frame = true;
    }
}

#[derive(Clone, Debug, Default)]
pub struct RenderVisiblePointCloudEntity {
    pub asset_id: AssetId<PointCloud>,
    pub chunk_entities: Vec<RenderVisiblePointCloudChunkEntity>,
}

#[derive(Clone, Debug)]
pub struct RenderVisiblePointCloudChunkEntity {
    /// the corresponding chunk entity, if available
    pub id: NodeId,
    pub chunk_id: AssetId<PointCloudChunk>,
    pub name: Arc<str>,
    pub parent_id: Option<NodeId>,
    pub depth: u32,
    pub child_index: ChildIndex,
    pub children: [usize; 8],
    pub children_mask: ChildrenMask,
    pub entity: Entity,
    pub main_entity: MainEntity,
}

// impl From<&VisiblePointCloudNodeEntity> for RenderVisiblePointCloudNodeEntity {
//     fn from(value: &VisiblePointCloudNodeEntity) -> Self {
//         RenderVisiblePointCloudNodeEntity {
//             id: value.id,
//             chunk_id: value.chunk_id,
//             name: value.name.clone(),
//             parent_id: value.parent_id,
//             depth: value.depth,
//             child_index: value.child_index,
//             children: value.children,
//             children_mask: value.children_mask,
//             entity: value.entity,
//         }
//     }
// }
