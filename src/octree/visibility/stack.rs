use std::cmp::Ordering;

use bevy_asset::AssetId;
use bevy_ecs::prelude::*;
use ordered_float::OrderedFloat;

use crate::octree::{
    asset::Octree,
    node::{NodeData, OctreeNode},
};

pub struct StackedOctreeNode<'a, T: NodeData> {
    pub entity: Entity,
    pub asset_id: AssetId<Octree<T>>,
    pub octree: &'a Octree<T>,
    pub node: &'a OctreeNode<T>,
    pub screen_pixel_radius: Option<f32>,
    pub weight: OrderedFloat<f32>,
    pub completely_visible: bool,
    pub parent_index: Option<usize>,
}

impl<'a, T: NodeData> Eq for StackedOctreeNode<'a, T> {}

impl<'a, T: NodeData> PartialEq<Self> for StackedOctreeNode<'a, T> {
    fn eq(&self, other: &Self) -> bool {
        self.weight.eq(&other.weight)
    }
}

impl<'a, T: NodeData> PartialOrd<Self> for StackedOctreeNode<'a, T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.weight.partial_cmp(&other.weight)
    }
}

impl<'a, T: NodeData> Ord for StackedOctreeNode<'a, T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.weight.cmp(&other.weight)
    }
}
