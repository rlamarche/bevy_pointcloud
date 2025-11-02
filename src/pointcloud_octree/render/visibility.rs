use bevy_camera::Camera;
use crate::octree::asset::NodeId;
use crate::pointcloud_octree::visibility::VisiblePointCloudOctree3dNodes;
use bevy_ecs::prelude::*;
use bevy_ecs::query::QueryItem;
use bevy_platform::collections::HashMap;
use bevy_reflect::{std_traits::ReflectDefault, Reflect};
use bevy_render::extract_component::ExtractComponent;
use bevy_log::prelude::*;

#[derive(Clone, Component, Default, Debug, Reflect)]
#[reflect(Component, Default, Debug, Clone)]
pub struct RenderVisiblePointCloudOctree3dNodes {
    #[reflect(ignore, clone)]
    pub octrees: HashMap<Entity, Vec<NodeId>>,
}

impl ExtractComponent for VisiblePointCloudOctree3dNodes {
    type QueryData = &'static Self;
    type QueryFilter = With<Camera>;
    type Out = RenderVisiblePointCloudOctree3dNodes;

    fn extract_component(
        (visible_octree_nodes): QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        Some(RenderVisiblePointCloudOctree3dNodes {
            octrees: visible_octree_nodes.nodes.clone(),
        })
    }
}
