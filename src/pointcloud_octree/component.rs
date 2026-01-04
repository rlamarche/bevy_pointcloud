use crate::octree::asset::Octree;
use super::asset::data::PointCloudNodeData;
use bevy_asset::{AssetId, Handle};
use bevy_ecs::prelude::*;
use bevy_ecs::query::QueryItem;
use bevy_render::extract_component::ExtractComponent;
use bevy_transform::prelude::GlobalTransform;
use crate::point_cloud_material::PointCloudMaterial3d;
use crate::pointcloud_octree::render::data::PointCloudOctree3dUniform;

#[derive(Component, Clone, Debug)]
pub struct PointCloudOctree3d(pub Handle<Octree<PointCloudNodeData>>);

impl Into<AssetId<Octree<PointCloudNodeData>>> for &PointCloudOctree3d {
    fn into(self) -> AssetId<Octree<PointCloudNodeData>> {
        self.0.clone().id()
    }
}

impl ExtractComponent for PointCloudOctree3d {
    type QueryData = (
        &'static PointCloudOctree3d,
        &'static GlobalTransform,
        &'static PointCloudMaterial3d,
    );
    type QueryFilter = ();
    type Out = (
        PointCloudOctree3d,
        PointCloudOctree3dUniform,
        PointCloudMaterial3d,
    );

    fn extract_component(
        (point_cloud_3d, global_transform, point_cloud_material_3d): QueryItem<
            '_,
            '_,
            Self::QueryData,
        >,
    ) -> Option<Self::Out> {
        let point_cloud_octree_3d_uniform = PointCloudOctree3dUniform {
            world_from_local: global_transform.to_matrix(),
        };
        Some((
            point_cloud_3d.clone(),
            point_cloud_octree_3d_uniform,
            point_cloud_material_3d.clone(),
        ))
    }
}
