use crate::octree::node::{NodeData, OctreeNode};

pub trait OctreeNodesBudget<T: NodeData>: Send + Sync + 'static {
    type Settings: Send + Sync;

    fn new(settings: &Self::Settings) -> Self;

    fn add_node(&mut self, node: &OctreeNode<T>) -> bool;

    fn value(&self) -> f64;
}

impl<T: NodeData> OctreeNodesBudget<T> for () {
    type Settings = ();

    fn new(_settings: &Self::Settings) -> Self {}

    fn add_node(&mut self, _node: &OctreeNode<T>) -> bool {
        true
    }

    fn value(&self) -> f64 {
        -1.0
    }
}

impl<T: NodeData, B: OctreeNodesBudget<T>> OctreeNodesBudget<T> for Option<B> {
    type Settings = B::Settings;

    fn new(settings: &Self::Settings) -> Self {
        Some(B::new(settings))
    }

    fn add_node(&mut self, node: &OctreeNode<T>) -> bool {
        match self {
            None => true,
            Some(this) => this.add_node(node),
        }
    }

    fn value(&self) -> f64 {
        match self {
            None => -1.0,
            Some(this) => this.value(),
        }
    }
}
