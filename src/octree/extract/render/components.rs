use std::marker::PhantomData;

use bevy_asset::AssetId;
use bevy_ecs::prelude::*;
use bevy_platform::collections::HashMap;
use bevy_render::render_resource::BindGroup;

use crate::octree::{asset::Octree, node::NodeData, visibility::components::VisibleOctreeNode};

/// This component stores the visible nodes for each octree at view level (camera) in "render world".
#[derive(Clone, Component, Debug)]
pub struct RenderVisibleOctreeNodes<T, C>
where
    T: NodeData,
    C: Component,
{
    /// The `Entity` used here refers to the "render world"
    pub(crate) octrees: HashMap<Entity, (AssetId<Octree<T>>, Vec<VisibleOctreeNode>)>,
    pub(crate) changed_this_frame: bool,
    pub(crate) _phantom_data: PhantomData<C>,
}

impl<T, C> Default for RenderVisibleOctreeNodes<T, C>
where
    T: NodeData,
    C: Component,
{
    fn default() -> Self {
        Self {
            octrees: Default::default(),
            changed_this_frame: false,
            _phantom_data: Default::default(),
        }
    }
}

#[derive(Component)]
pub struct RenderOctreeEntityUniform<T, C> {
    pub(crate) bind_group: BindGroup,
    pub(crate) _phantom: PhantomData<fn() -> (T, C)>,
}
