use std::sync::Arc;

use bevy::{
    asset::AssetId,
    ecs::{component::Component, entity::Entity},
    platform::collections::HashMap,
    render::{
        render_resource::BindGroup,
        sync_world::{MainEntity, RenderEntity},
        texture::ColorAttachment,
        view::RetainedViewEntity,
    },
};

use crate::{ChildIndex, ChildrenMask, NodeId, PointCloud, PointCloudChunk};

/// This component stores the visible nodes for each point cloud at view level (camera) in "render
/// world".
#[derive(Debug, Component, Default, Clone)]
pub struct RenderVisiblePointCloudEntities {
    pub entities: HashMap<MainEntity, RenderVisiblePointCloudEntity>,
    pub changed_this_frame: bool,
}

impl RenderVisiblePointCloudEntities {
    pub fn get(&self, main_entity: &MainEntity) -> Option<&RenderVisiblePointCloudEntity> {
        self.entities.get(main_entity)
    }

    pub fn get_or_insert_mut(
        &mut self,
        main_entity: MainEntity,
        render_entity: RenderEntity,
        asset_id: AssetId<PointCloud>,
    ) -> &mut RenderVisiblePointCloudEntity {
        let entry =
            self.entities
                .entry(main_entity)
                .or_insert_with(|| RenderVisiblePointCloudEntity {
                    entity: render_entity.id(),
                    asset_id,
                    chunk_entities: Vec::new(),
                });
        entry.entity = render_entity.id();
        entry.asset_id = asset_id;
        entry
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

#[derive(Clone, Component, Default, Debug)]
pub struct RenderShadowMapVisiblePointCloudEntities {
    /// A mapping from each subview (cascade or cubemap face) to the point cloud entities
    /// visible from it.
    pub subviews: HashMap<RetainedViewEntity, RenderVisiblePointCloudEntities>,
}

#[derive(Clone, Debug)]
pub struct RenderVisiblePointCloudEntity {
    /// a render entity
    pub entity: Entity,
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
    /// offset applied to point size
    pub offset: u8,
    pub child_index: ChildIndex,
    pub first_child_index: usize,
    pub children: [usize; 8],
    pub children_mask: ChildrenMask,
    pub entity: Entity,
    pub main_entity: MainEntity,
}

/// Stores visible nodes and mapping textures for each view
#[derive(Component)]
pub struct VisibleNodesTexture {
    pub visible_nodes: Option<ColorAttachment>,
    /// contains node index per octree index (see [`crate::RenderOctreeInstancesIndex`])
    pub node_index: Vec<HashMap<NodeId, u32>>,
}

#[derive(Component)]
pub struct VisibleNodesTextureBindGroup {
    pub texture: BindGroup,
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
