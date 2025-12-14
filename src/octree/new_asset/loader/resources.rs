use crate::octree::new_asset::asset::NewOctree;
use crate::octree::new_asset::hierarchy::HierarchyNodeData;
use crate::octree::storage::NodeId;
use bevy_asset::AssetId;
use bevy_ecs::prelude::*;
use bevy_platform::collections::HashSet;
use ordered_float::OrderedFloat;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::marker::PhantomData;
use crate::octree::new_asset::node::NodeData;

#[derive(Resource)]
pub struct OctreeLoadTasks<H, T>
where
    H: HierarchyNodeData,
    T: NodeData,
{
    pub hierarchy_heap: BinaryHeap<OctreeNodeLoadTask<H, T>>,
    pub node_heap: BinaryHeap<OctreeNodeLoadTask<H, T>>,
    pub hierarchy_in_flight: HashSet<(AssetId<NewOctree<H, T>>, NodeId)>,
    pub node_in_flight: HashSet<(AssetId<NewOctree<H, T>>, NodeId)>,
    _phantom_data: PhantomData<fn() -> (H, T)>,
}

#[derive(Clone, Debug)]
pub enum LoadRequestType {
    Hierarchy,
    NodeData,
}

impl<H, T> OctreeLoadTasks<H, T>
where
    H: HierarchyNodeData,
    T: NodeData,
{
    pub fn queue_load_request(
        &mut self,
        asset_id: AssetId<NewOctree<H, T>>,
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
impl<H, T> Default for OctreeLoadTasks<H, T>
where
    H: HierarchyNodeData,
    T: NodeData,
{
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
pub struct OctreeNodeLoadTask<H, T>
where
    H: HierarchyNodeData,
    T: NodeData,
{
    pub asset_id: AssetId<NewOctree<H, T>>,
    pub node_id: NodeId,
    pub weight: OrderedFloat<f32>,
}

impl<H, T> Eq for OctreeNodeLoadTask<H, T>
where
    H: HierarchyNodeData,
    T: NodeData,
{
}

impl<H, T> PartialEq<Self> for OctreeNodeLoadTask<H, T>
where
    H: HierarchyNodeData,
    T: NodeData,
{
    fn eq(&self, other: &Self) -> bool {
        self.weight == other.weight
    }
}

impl<H, T> PartialOrd<Self> for OctreeNodeLoadTask<H, T>
where
    H: HierarchyNodeData,
    T: NodeData,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.weight.partial_cmp(&other.weight)
    }
}

impl<H, T> Ord for OctreeNodeLoadTask<H, T>
where
    H: HierarchyNodeData,
    T: NodeData,
{
    fn cmp(&self, other: &Self) -> Ordering {
        self.weight.cmp(&other.weight)
    }
}
