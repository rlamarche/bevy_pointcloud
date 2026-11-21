use std::{any::TypeId, cmp::Ordering, hash::Hash};

use bevy::{
    ecs::{component::Component, entity::Entity},
    platform::{
        collections::{HashMap, HashSet},
        hash::NoOpHash,
    },
    render::{
        render_resource::BindGroup,
        sync_world::{MainEntity, MainEntityHashMap},
        texture::CachedTexture,
        view::RetainedViewEntity,
    },
};

use crate::{NodeId, PointCloudChunkTopology};

pub type ChunkKey = (MainEntity, MainEntity);

/// This component stores the visible nodes for each point cloud at view level (camera) in "render
/// world".
#[derive(Debug, Component, Default, Clone)]
pub struct RenderVisiblePointCloudEntities {
    // Indexed by `root_entity`; each `Vec` is kept sorted by `(depth, name)`.
    pub entities: MainEntityHashMap<Vec<RenderVisiblePointCloudChunkEntity>>,
    // Reused across frames via `.clear()` to avoid heap allocations.
    pub added_this_frame: HashSet<RenderVisiblePointCloudChunkEntity>,
    pub removed_this_frame: HashSet<ChunkKey>,
    pub changed_this_frame: bool,
}

impl RenderVisiblePointCloudEntities {
    pub fn prepare_for_new_frame(&mut self) {
        self.added_this_frame.clear();
        self.removed_this_frame.clear();
        self.changed_this_frame = false;
    }

    pub fn add_visible_chunk(&mut self, chunk: RenderVisiblePointCloudChunkEntity) {
        self.added_this_frame.replace(chunk);
    }

    pub fn remove_visible_chunk(&mut self, root_entity: MainEntity, main_entity: MainEntity) {
        self.removed_this_frame.insert((root_entity, main_entity));
    }

    pub fn update_visible_chunks(&mut self) {
        if self.added_this_frame.is_empty() && self.removed_this_frame.is_empty() {
            self.changed_this_frame = false;
            return;
        }

        // Remove chunks that were not also added this frame (`add` always wins over `remove`).
        for &(root_entity, main_entity) in &self.removed_this_frame {
            let was_added = self
                .added_this_frame
                .iter()
                .any(|c| c.root_entity == root_entity && c.main_entity == main_entity);

            if !was_added && let Some(chunks) = self.entities.get_mut(&root_entity) {
                chunks.retain(|c| c.main_entity != main_entity);
            }
        }

        // Insert or update added chunks while preventing duplicates.
        for chunk in self.added_this_frame.drain() {
            let chunks = self.entities.entry(chunk.root_entity).or_default();
            if let Some(existing) = chunks.iter_mut().find(|c| *c == &chunk) {
                *existing = chunk;
            } else {
                chunks.push(chunk);
            }
        }

        self.removed_this_frame.clear();

        // Drop point clouds with no visible chunks left, and re-sort active ones.
        self.entities.retain(|_, chunks| !chunks.is_empty());
        for chunks in self.entities.values_mut() {
            chunks.sort_unstable_by(|a, b| {
                let (Some(a), Some(b)) = (a.topology.as_octree(), b.topology.as_octree()) else {
                    return Ordering::Equal;
                };

                a.depth.cmp(&b.depth).then_with(|| a.name.cmp(&b.name))
            });
        }

        self.changed_this_frame = true;
    }

    pub fn get(&self, main_entity: &MainEntity) -> Option<&[RenderVisiblePointCloudChunkEntity]> {
        self.entities.get(main_entity).map(Vec::as_slice)
    }

    pub fn clear_all(&mut self) {
        let had_items = self.entities.values().any(|chunks| !chunks.is_empty());
        // Keep `Vec` and map capacities so allocations are reused on subsequent frames.
        for chunks in self.entities.values_mut() {
            chunks.clear();
        }
        self.entities.retain(|_, chunks| !chunks.is_empty());
        if had_items {
            self.changed_this_frame = true;
        }
    }
}

#[derive(Clone, Component, Default, Debug)]
pub struct RenderShadowMapVisiblePointCloudEntities {
    /// A mapping from each subview (cascade or cubemap face) to the point cloud entities
    /// visible from it.
    pub subviews: HashMap<RetainedViewEntity, RenderVisiblePointCloudEntities>,
}

impl RenderShadowMapVisiblePointCloudEntities {
    pub fn prepare_for_new_frame(&mut self) {
        for item in self.subviews.values_mut() {
            item.prepare_for_new_frame();
        }
    }

    pub fn update_visible_chunks(&mut self) {
        for item in self.subviews.values_mut() {
            item.update_visible_chunks();
        }
    }
}

#[derive(Clone, Debug)]
pub struct RenderVisiblePointCloudChunkEntity {
    pub topology: PointCloudChunkTopology,
    pub entity: Entity,
    // the chunk's main entity
    pub main_entity: MainEntity,
    // the point cloud's main entity
    pub root_entity: MainEntity,
}

impl PartialEq for RenderVisiblePointCloudChunkEntity {
    fn eq(&self, other: &Self) -> bool {
        self.main_entity == other.main_entity && self.root_entity == other.root_entity
    }
}

impl Eq for RenderVisiblePointCloudChunkEntity {}

impl Hash for RenderVisiblePointCloudChunkEntity {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.root_entity.hash(state);
        self.main_entity.hash(state);
    }
}

/// Stores visible nodes and mapping textures for each view
#[derive(Component, Default)]
pub struct VisibleNodesTexture {
    pub texture: Option<CachedTexture>,
    /// contains node index per octree index (see [`crate::RenderOctreeInstancesIndex`])
    pub node_index: Vec<HashMap<NodeId, u32>>,
    /// if true, needs to reallocate the associated bind group
    pub has_changed: bool,
}

#[derive(Component, Clone, Default)]
pub struct ViewPointCloudBindGroups {
    // contains one bind group per material (TypeId) and per point cloud entity
    pub bind_groups: HashMap<TypeId, MainEntityHashMap<BindGroup>, NoOpHash>,
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
