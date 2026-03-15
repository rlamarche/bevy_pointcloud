use std::marker::PhantomData;

use bevy_asset::AssetId;
use bevy_ecs::prelude::*;
use bevy_platform::collections::HashMap;

use crate::octree::{asset::Octree, node::NodeData, visibility::components::VisibleOctreeNode};

/// This component stores the visible nodes for each octree at view level (camera) in "render world".
#[derive(Clone, Component, Default, Debug)]
pub struct RenderVisibleOctreeNodes<T, C>
where
    T: NodeData,
    C: Component,
{
    /// The `Entity` used here refers to the "render world"
    pub(crate) octrees: HashMap<Entity, (AssetId<Octree<T>>, Vec<VisibleOctreeNode>)>,
    pub(crate) _phantom_data: PhantomData<C>,
}
