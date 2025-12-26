use crate::octree::new_asset::asset::NewOctree;
use crate::pointcloud_octree::asset::PointCloudNodeData;
use bevy_asset::{AssetId, Handle};
use bevy_ecs::prelude::*;
use bevy_ecs::query::QueryItem;
use bevy_render::extract_component::ExtractComponent;
use bevy_transform::prelude::GlobalTransform;
use crate::point_cloud_material::PointCloudMaterial3d;
use crate::pointcloud_octree::new_asset::render::data::NewPointCloudOctree3dUniform;

#[derive(Component, Clone, Debug)]
pub struct NewPointCloudOctree3d(pub Handle<NewOctree<PointCloudNodeData>>);

impl Into<AssetId<NewOctree<PointCloudNodeData>>> for &NewPointCloudOctree3d {
    fn into(self) -> AssetId<NewOctree<PointCloudNodeData>> {
        self.0.clone().id()
    }
}

impl ExtractComponent for NewPointCloudOctree3d {
    type QueryData = (
        &'static NewPointCloudOctree3d,
        &'static GlobalTransform,
        &'static PointCloudMaterial3d,
    );
    type QueryFilter = ();
    type Out = (
        NewPointCloudOctree3d,
        NewPointCloudOctree3dUniform,
        PointCloudMaterial3d,
    );

    fn extract_component(
        (point_cloud_3d, global_transform, point_cloud_material_3d): QueryItem<
            '_,
            '_,
            Self::QueryData,
        >,
    ) -> Option<Self::Out> {
        let point_cloud_octree_3d_uniform = NewPointCloudOctree3dUniform {
            world_from_local: global_transform.to_matrix(),
        };
        Some((
            point_cloud_3d.clone(),
            point_cloud_octree_3d_uniform,
            point_cloud_material_3d.clone(),
        ))
    }
}
