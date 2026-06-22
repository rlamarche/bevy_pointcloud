use bevy_ecs::query::QueryItem;
use bevy_render::extract_component::ExtractComponent;
use bevy_transform::prelude::GlobalTransform;

use crate::{
    point::Point,
    point_cloud::PointCloud3d,
    point_cloud_material::{PointCloudMaterial, PointCloudMaterial3d},
    render::point_cloud_uniform::PointCloudUniform,
};

impl<T: Point> ExtractComponent for PointCloud3d<T> {
    type QueryData = (&'static PointCloud3d<T>, &'static GlobalTransform);
    type QueryFilter = ();
    type Out = (PointCloud3d<T>, PointCloudUniform);

    fn extract_component(
        (point_cloud_3d, global_transform): QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        let custom_uniform = PointCloudUniform {
            world_from_local: global_transform.to_matrix(),
        };
        Some((point_cloud_3d.clone(), custom_uniform))
    }
}

impl<M: PointCloudMaterial> ExtractComponent for PointCloudMaterial3d<M> {
    type QueryData = &'static PointCloudMaterial3d<M>;
    type QueryFilter = ();
    type Out = PointCloudMaterial3d<M>;

    fn extract_component(
        point_cloud_material_3d: QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        Some(point_cloud_material_3d.clone())
    }
}
