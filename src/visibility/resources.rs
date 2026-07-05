use bevy::{
    asset::AssetId,
    ecs::{
        entity::{EntityHash, EntityHashMap},
        prelude::*,
    },
};
use indexmap::IndexMap;
use ordered_float::OrderedFloat;

use crate::{PointCloud, PointCloudNode, PointCloudNodeKey};

/// This resource contains all visible point cloud nodes in the current iteration, across all
/// cameras
#[derive(Resource, Default)]
pub struct GlobalVisiblePointCloudNodes {
    pub visible_nodes: IndexMap<PointCloudNodeKey, OrderedFloat<f32>>,
}

impl GlobalVisiblePointCloudNodes {
    pub fn clear(&mut self) {
        self.visible_nodes.clear();
    }

    pub fn add_visible_node(
        &mut self,
        id: AssetId<PointCloud>,
        node: &PointCloudNode,
        weight: OrderedFloat<f32>,
    ) {
        self.visible_nodes.insert(
            PointCloudNodeKey {
                id,
                node_id: node.id,
            },
            weight,
        );
    }
}

/// This resource contains all visible point cloud chunk instances (entities) visible in the current
/// iteration, per point cloud instances. It is used to determine quickly which needs
/// sepecialization. They are sorted by priority (not sure to keep this unneeded yet order).
/// Note that because we keep allocations, if a key exists for a given point cloud instance, it does
/// not necessarly means it is visible. Must check for chunks.
#[derive(Resource, Default)]
pub struct GlobalVisiblePointCloudChunks {
    pub visible_point_clouds: EntityHashMap<IndexMap<Entity, OrderedFloat<f32>, EntityHash>>,
}

impl GlobalVisiblePointCloudChunks {
    pub fn clear(&mut self) {
        for visible_chunks in self.visible_point_clouds.values_mut() {
            // we keep allocations
            // TODO clean on point cloud removed
            visible_chunks.clear();
        }
    }

    pub fn get(&self, entity: &Entity) -> Option<&IndexMap<Entity, OrderedFloat<f32>, EntityHash>> {
        self.visible_point_clouds.get(entity)
    }

    pub fn add_visible_chunk(
        &mut self,
        point_cloud_entity: Entity,
        chunk_entity: Entity,
        weight: OrderedFloat<f32>,
    ) {
        self.visible_point_clouds
            .entry(point_cloud_entity)
            .or_default()
            .insert(chunk_entity, weight);
    }
}
