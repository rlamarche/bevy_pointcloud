use crate::octree::asset::Octree;
use crate::octree::hierarchy::HierarchyNodeData;
use crate::octree::node::NodeData;
use crate::octree::storage::NodeId;
use bevy_asset::AssetId;
use bevy_ecs::prelude::*;
use bevy_platform::collections::HashSet;
use ordered_float::OrderedFloat;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::marker::PhantomData;

#[derive(Resource)]
pub struct OctreeLoadTasks<T: NodeData> {
    pub hierarchy_heap: BinaryHeap<OctreeNodeLoadTask<T>>,
    pub node_heap: BinaryHeap<OctreeNodeLoadTask<T>>,
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
                    self.hierarchy_heap.push(OctreeNodeLoadTask {
                        asset_id,
                        node_id,
                        weight,
                    });
                }
            }
            LoadRequestType::NodeData => {
                if !self.node_in_flight.contains(&key) {
                    self.node_heap.push(OctreeNodeLoadTask {
                        asset_id,
                        node_id,
                        weight,
                    });
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
    pub weight: OrderedFloat<f32>,
}

impl<T: NodeData> Eq for OctreeNodeLoadTask<T> {}

impl<T: NodeData> PartialEq<Self> for OctreeNodeLoadTask<T> {
    fn eq(&self, other: &Self) -> bool {
        self.weight == other.weight
    }
}

impl<T: NodeData> PartialOrd<Self> for OctreeNodeLoadTask<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.weight.partial_cmp(&other.weight)
    }
}

impl<T: NodeData> Ord for OctreeNodeLoadTask<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.weight.cmp(&other.weight)
    }
}
