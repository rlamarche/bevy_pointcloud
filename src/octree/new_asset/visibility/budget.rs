use crate::octree::new_asset::hierarchy::HierarchyNodeData;
use crate::octree::new_asset::node::{NodeData, OctreeNode};
use bevy_ecs::prelude::*;
use bevy_log::prelude::*;
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum BudgetError {
    #[error("Budget for octree hierarchy has been reached.")]
    NoBudgetLeft,
}

pub trait OctreeHierarchyBudget<H, T>: Send + Sync
where
    H: HierarchyNodeData,
    T: NodeData,
{
    type Settings: Send + Sync;

    fn new(settings: Self::Settings) -> Self;

    fn check(&self, node: &OctreeNode<H, T>) -> bool;

    fn add_node(&mut self, node: &OctreeNode<H, T>) -> Result<(), BudgetError>;
}
