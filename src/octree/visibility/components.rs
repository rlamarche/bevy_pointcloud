use crate::octree::{
    asset::Octree,
    node::{NodeData, OctreeNode},
    storage::NodeId,
    visibility::{budget::OctreeNodesBudget, filter::OctreeNodesFilter},
};
use bevy_asset::AssetId;
use bevy_ecs::prelude::*;
use bevy_platform::collections::HashMap;
use bevy_reflect::Reflect;
use std::{marker::PhantomData, sync::Arc};

#[derive(Debug, Component, Reflect)]
pub struct OctreeVisibilitySettings<T: NodeData, F: OctreeNodesFilter<T>, B: OctreeNodesBudget<T>> {
    pub filter: Option<F::Settings>,
    pub budget: Option<B::Settings>,
}

/// This component stores the visible nodes for each octree at view level (camera) in "main world".
#[derive(Debug, Component)]
pub struct ViewVisibleOctreeNodes<T, C>
where
    T: NodeData,
    C: Component,
{
    pub octrees: HashMap<Entity, (AssetId<Octree<T>>, Vec<VisibleOctreeNode>)>,
    pub changed_this_frame: bool,
    _phantom_data: PhantomData<fn() -> (T, C)>,
}

impl<T, C> Default for ViewVisibleOctreeNodes<T, C>
where
    T: NodeData,
    C: Component,
{
    fn default() -> Self {
        Self {
            octrees: HashMap::default(),
            changed_this_frame: false,
            _phantom_data: PhantomData,
        }
    }
}

impl<T, C> ViewVisibleOctreeNodes<T, C>
where
    T: NodeData,
    C: Component,
{
    pub fn get_mut(&mut self, entity: Entity) -> &mut (AssetId<Octree<T>>, Vec<VisibleOctreeNode>) {
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
    pub asset_id: AssetId<Octree<T>>,
    pub visible_nodes: Vec<VisibleOctreeNode>,
    _phantom_data: PhantomData<fn() -> (T, C)>,
}

/// This component stores the visible nodes for each octree at view level (camera) in "main world".
#[derive(Component)]
pub struct SkipOctreeVisibility;

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

impl<T: NodeData> From<&OctreeNode<T>> for VisibleOctreeNode {
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
