use crate::octree::new_asset::node::{NodeData, OctreeNode};
use bevy_ecs::prelude::*;
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum BudgetError {
    #[error("Budget for octree hierarchy has been reached.")]
    NoBudgetLeft,
}

pub trait OctreeHierarchyBudget<T: NodeData>: Send + Sync
{
    type Settings: Send + Sync;

    fn new(settings: Self::Settings) -> Self;

    fn check(&self, node: &OctreeNode<T>) -> bool;

    fn add_node(&mut self, node: &OctreeNode<T>) -> Result<(), BudgetError>;
}
