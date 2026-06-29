use bevy::{asset::AssetId, ecs::prelude::*};
use indexmap::IndexMap;
use ordered_float::OrderedFloat;

use crate::{PointCloud, PointCloudNode, PointCloudNodeKey};

/// This resource contains all visible octree nodes in the current iteration, across all cameras
#[derive(Resource)]
pub struct GlobalVisiblePointCloudNodes {
    pub(crate) visible_nodes: IndexMap<PointCloudNodeKey, OrderedFloat<f32>>,
}

impl Default for GlobalVisiblePointCloudNodes {
    fn default() -> Self {
        Self {
            visible_nodes: IndexMap::new(),
        }
    }
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
