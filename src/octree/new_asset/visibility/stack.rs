use crate::octree::new_asset::asset::NewOctree;
use crate::octree::new_asset::hierarchy::{
    HierarchyNodeData, HierarchyOctreeNode,
};
use bevy_ecs::prelude::*;
use bevy_reflect::TypePath;
use ordered_float::OrderedFloat;
use std::cmp::Ordering;

pub struct StackedOctreeNode<'a, H, T>
where
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
{
    pub octree: &'a NewOctree<H, T>,
    pub entity: Entity,
    pub node: &'a HierarchyOctreeNode<H>,
    pub screen_pixel_radius: Option<f32>,
    pub weight: OrderedFloat<f32>,
    pub completely_visible: bool,
    pub parent_index: Option<usize>,
}

impl<'a, H, T> Eq for StackedOctreeNode<'a, H, T>
where
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
{
}

impl<'a, H, T> PartialEq<Self> for StackedOctreeNode<'a, H, T>
where
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
{
    fn eq(&self, other: &Self) -> bool {
        self.weight.eq(&other.weight)
    }
}

impl<'a, H, T> PartialOrd<Self> for StackedOctreeNode<'a, H, T>
where
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.weight.partial_cmp(&other.weight)
    }
}

impl<'a, H, T> Ord for StackedOctreeNode<'a, H, T>
where
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
{
    fn cmp(&self, other: &Self) -> Ordering {
        self.weight.cmp(&other.weight)
    }
}
