use crate::octree::new_asset::asset::NewOctree;
use crate::octree::new_asset::hierarchy::HierarchyNodeData;
use crate::octree::new_asset::node::{NodeData, OctreeNode};
use bevy_ecs::prelude::*;
use ordered_float::OrderedFloat;
use std::cmp::Ordering;

pub struct StackedOctreeNode<'a, H, T>
where
    H: HierarchyNodeData,
    T: NodeData,
{
    pub octree: &'a NewOctree<H, T>,
    pub entity: Entity,
    pub node: &'a OctreeNode<H, T>,
    pub screen_pixel_radius: Option<f32>,
    pub weight: OrderedFloat<f32>,
    pub completely_visible: bool,
    pub parent_index: Option<usize>,
}

impl<'a, H, T> Eq for StackedOctreeNode<'a, H, T>
where
    H: HierarchyNodeData,
    T: NodeData,
{
}

impl<'a, H, T> PartialEq<Self> for StackedOctreeNode<'a, H, T>
where
    H: HierarchyNodeData,
    T: NodeData,
{
    fn eq(&self, other: &Self) -> bool {
        self.weight.eq(&other.weight)
    }
}

impl<'a, H, T> PartialOrd<Self> for StackedOctreeNode<'a, H, T>
where
    H: HierarchyNodeData,
    T: NodeData,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.weight.partial_cmp(&other.weight)
    }
}

impl<'a, H, T> Ord for StackedOctreeNode<'a, H, T>
where
    H: HierarchyNodeData,
    T: NodeData,
{
    fn cmp(&self, other: &Self) -> Ordering {
        self.weight.cmp(&other.weight)
    }
}
