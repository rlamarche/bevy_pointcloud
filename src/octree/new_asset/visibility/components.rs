use crate::octree::new_asset::asset::NewOctree;
use crate::octree::new_asset::hierarchy::HierarchyNodeData;
use crate::octree::new_asset::node::{NodeData, OctreeNode};
use crate::octree::storage::NodeId;
use bevy_asset::AssetId;
use bevy_ecs::prelude::*;
use bevy_platform::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

/// This component stores the visible nodes for each octree at view level (camera) in "main world".
#[derive(Debug, Component)]
pub struct OctreesVisibility<T, C>
where
    T: NodeData,
    C: Component,
{
    pub octrees: HashMap<Entity, (AssetId<NewOctree<T>>, Vec<VisibleOctreeNode>)>,
    _phantom_data: PhantomData<fn() -> (T, C)>,
}

impl<T, C> Default for OctreesVisibility<T, C>
where
    T: NodeData,
    C: Component,
{
    fn default() -> Self {
        Self {
            octrees: HashMap::default(),
            _phantom_data: PhantomData,
        }
    }
}

impl<T, C> OctreesVisibility<T, C>
where
    T: NodeData,
    C: Component,
{
    pub fn get_mut(
        &mut self,
        entity: Entity,
    ) -> &mut (AssetId<NewOctree<T>>, Vec<VisibleOctreeNode>) {
        self.octrees.entry(entity).or_default()
    }

    pub fn clear_all(&mut self) {
        // Don't just nuke the hash table; we want to reuse allocations.
        for (asset_id, nodes) in self.octrees.values_mut() {
            *asset_id = Default::default();
            nodes.clear();
        }
    }
}

/// This component stores the visible nodes for each octree at view level (camera) in "main world".
#[derive(Debug, Component)]
pub struct OctreeVisibility<T, C>
where
    T: NodeData,
    C: Component,
{
    pub asset_id: AssetId<NewOctree<T>>,
    pub visible_nodes: Vec<VisibleOctreeNode>,
    _phantom_data: PhantomData<fn() -> (T, C)>,
}

// #[derive(Clone, Debug)]
// pub struct OctreeVisibility<H, T>
// where
//     H: HierarchyNodeData,
//     T: NodeData,
// {
//     pub asset_id: AssetId<NewOctree<H, T>>,
//     pub visible_nodes: Vec<VisibleOctreeNode>,
//     pub visible_hierarchy_nodes_to_load: Vec<NodeId>,
//     pub visible_nodes_to_load: Vec<NodeId>,
// }

/// Contains useful informations about a visible node
#[derive(Clone, Debug)]
pub struct VisibleOctreeNode {
    pub id: NodeId,
    pub name: Arc<str>,
    pub parent_id: Option<NodeId>,
    pub depth: u32,
    pub child_index: u8,
    pub children: [usize; 8],
    pub children_mask: u8,
}

// pub struct VisibleHierarchyNode<H>
// where
//     H: HierarchyNodeData,
// {
//     pub index: usize,
//     pub children: [usize; 8],
//     pub children_mask: u8,
//     pub node: HierarchyOctreeNode<H>,
// }

impl<T: NodeData> From<&OctreeNode<T>> for VisibleOctreeNode
{
    fn from(value: &OctreeNode<T>) -> Self {
        VisibleOctreeNode {
            id: value.hierarchy.id,
            name: value.hierarchy.name.clone(),
            parent_id: value.hierarchy.parent_id,
            depth: value.hierarchy.depth,
            child_index: value.hierarchy.child_index,
            children: [0_usize; 8],
            children_mask: 0b00000000,
        }
    }
}
