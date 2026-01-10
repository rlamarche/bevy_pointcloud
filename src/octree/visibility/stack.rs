use crate::octree::asset::Octree;
use crate::octree::node::NodeData;
use bevy_ecs::prelude::*;
use ordered_float::OrderedFloat;
use std::cmp::Ordering;
use bevy_asset::AssetId;
use crate::octree::storage::NodeId;

pub struct StackedOctreeNode<T: NodeData> {
    pub entity: Entity,
    pub asset_id: AssetId<Octree<T>>,
    pub node_id: NodeId,
    pub screen_pixel_radius: Option<f32>,
    pub weight: OrderedFloat<f32>,
    pub completely_visible: bool,
    pub parent_index: Option<usize>,
}

impl<T: NodeData> Eq for StackedOctreeNode<T> {}

impl<T: NodeData> PartialEq<Self> for StackedOctreeNode<T> {
    fn eq(&self, other: &Self) -> bool {
        self.weight.eq(&other.weight)
    }
}

impl<T: NodeData> PartialOrd<Self> for StackedOctreeNode<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.weight.partial_cmp(&other.weight)
    }
}

impl<T: NodeData> Ord for StackedOctreeNode<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.weight.cmp(&other.weight)
    }
}
