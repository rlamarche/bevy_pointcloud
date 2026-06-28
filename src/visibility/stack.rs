use std::cmp::Ordering;

use bevy::{asset::AssetId, ecs::entity::Entity};
use ordered_float::OrderedFloat;

use crate::{HierarchyNode, PointCloud};

#[derive(Clone, Debug)]
pub struct StackedPointCloudNodeEntity<'a> {
    /// the chunk entity
    pub entity: Entity,
    pub asset_id: AssetId<PointCloud>,
    pub octree: &'a PointCloud,
    pub node: &'a HierarchyNode,
    pub screen_pixel_radius: Option<f32>,
    pub weight: OrderedFloat<f32>,
    pub completely_visible: bool,
    pub parent_index: Option<usize>,
}

impl<'a> Eq for StackedPointCloudNodeEntity<'a> {}

impl<'a> PartialEq<Self> for StackedPointCloudNodeEntity<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.weight.eq(&other.weight)
    }
}

impl<'a> PartialOrd<Self> for StackedPointCloudNodeEntity<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> Ord for StackedPointCloudNodeEntity<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.weight.cmp(&other.weight)
    }
}
