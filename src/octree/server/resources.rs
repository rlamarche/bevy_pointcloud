use crate::octree::asset::Octree;
use crate::octree::node::NodeData;
use crate::octree::storage::NodeId;
use bevy_asset::AssetId;
use bevy_ecs::prelude::*;
use bevy_platform::collections::HashSet;
use ordered_float::OrderedFloat;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

#[derive(Resource)]
pub struct OctreeLoadTasks<T: NodeData> {
    pub hierarchy_heap: BinaryHeap<WeightedOctreeNodeLoadTask<T>>,
    pub node_heap: BinaryHeap<WeightedOctreeNodeLoadTask<T>>,
    pub hierarchy_in_flight: HashSet<(AssetId<Octree<T>>, NodeId)>,
    pub node_in_flight: HashSet<(AssetId<Octree<T>>, NodeId)>,
    _phantom_data: PhantomData<fn() -> T>,
}

#[derive(Clone, Debug)]
pub enum LoadRequestType {
    Hierarchy,
    NodeData,
}

impl<T: NodeData> OctreeLoadTasks<T> {
    pub fn queue_load_request(
        &mut self,
        asset_id: AssetId<Octree<T>>,
        node_id: NodeId,
        weight: OrderedFloat<f32>,
        request_type: LoadRequestType,
    ) {
        let key = (asset_id, node_id);

        match request_type {
            LoadRequestType::Hierarchy => {
                if !self.hierarchy_in_flight.contains(&key) {
                    self.hierarchy_heap.push(WeightedOctreeNodeLoadTask(
                        OctreeNodeLoadTask { asset_id, node_id },
                        weight,
                    ));
                }
            }
            LoadRequestType::NodeData => {
                if !self.node_in_flight.contains(&key) {
                    self.node_heap.push(WeightedOctreeNodeLoadTask(
                        OctreeNodeLoadTask { asset_id, node_id },
                        weight,
                    ));
                }
            }
        }
    }
}
impl<T: NodeData> Default for OctreeLoadTasks<T> {
    fn default() -> Self {
        Self {
            hierarchy_heap: Default::default(),
            node_heap: Default::default(),
            hierarchy_in_flight: Default::default(),
            node_in_flight: Default::default(),
            _phantom_data: Default::default(),
        }
    }
}

#[derive(Debug, Component)]
pub struct OctreeNodeLoadTask<T: NodeData> {
    pub asset_id: AssetId<Octree<T>>,
    pub node_id: NodeId,
}

impl<T: NodeData> Hash for OctreeNodeLoadTask<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.asset_id.hash(state);
        self.node_id.hash(state);
    }
}

impl<T: NodeData> PartialEq<Self> for OctreeNodeLoadTask<T> {
    fn eq(&self, other: &Self) -> bool {
        self.asset_id == other.asset_id && self.node_id == other.node_id
    }
}

impl<T: NodeData> Eq for OctreeNodeLoadTask<T> {}

pub struct WeightedOctreeNodeLoadTask<T: NodeData>(pub OctreeNodeLoadTask<T>, pub OrderedFloat<f32>);

impl<T: NodeData> PartialEq<Self> for WeightedOctreeNodeLoadTask<T> {
    fn eq(&self, other: &Self) -> bool {
        self.1.eq(&other.1)
    }
}

impl<T: NodeData> Eq for WeightedOctreeNodeLoadTask<T> {}

impl<T: NodeData> PartialOrd<Self> for WeightedOctreeNodeLoadTask<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.1.partial_cmp(&other.1)
    }
}

impl<T: NodeData> Ord for WeightedOctreeNodeLoadTask<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.1.cmp(&other.1)
    }
}
