use crate::octree::new_asset::hierarchy::{
    HierarchyNodeData, HierarchyOctreeNode,
};
use bevy_ecs::prelude::*;
use bevy_log::prelude::*;
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum BudgetError {
    #[error("Budget for octree hierarchy has been reached.")]
    NoBudgetLeft,
}

pub trait OctreeHierarchyBudget<H>: Send + Sync
where
    H: HierarchyNodeData,
{
    type Settings: Send + Sync;

    fn new(settings: Self::Settings) -> Self;

    fn check(&self, node: &HierarchyOctreeNode<H>) -> bool;

    fn add_node(&mut self, node: &HierarchyOctreeNode<H>) -> Result<(), BudgetError>;
}
