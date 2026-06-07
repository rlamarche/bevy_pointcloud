use bevy_asset::{AssetId, Handle};
use bevy_ecs::{prelude::*, query::QueryItem};
use bevy_render::extract_component::ExtractComponent;
use bevy_transform::prelude::GlobalTransform;

use super::asset::data::PointCloudNodeData;
use crate::{
    octree::asset::Octree, point::Point, pointcloud_octree::render::data::PointCloudOctree3dUniform,
};

#[derive(Component, Clone, Debug)]
pub struct PointCloudOctree3d<T: Point>(pub Handle<Octree<PointCloudNodeData<T>>>);

impl<T: Point> From<&PointCloudOctree3d<T>> for AssetId<Octree<PointCloudNodeData<T>>> {
    fn from(val: &PointCloudOctree3d<T>) -> Self {
        val.0.clone().id()
    }
}

impl<T: Point> ExtractComponent for PointCloudOctree3d<T> {
    type QueryData = (&'static PointCloudOctree3d<T>, &'static GlobalTransform);
    type QueryFilter = ();
    type Out = (PointCloudOctree3d<T>, PointCloudOctree3dUniform);

    fn extract_component(
        (point_cloud_3d, global_transform): QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        let point_cloud_octree_3d_uniform = PointCloudOctree3dUniform {
            world_from_local: global_transform.to_matrix(),
        };
        Some((point_cloud_3d.clone(), point_cloud_octree_3d_uniform))
    }
}
