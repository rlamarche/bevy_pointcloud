use crate::point_cloud::PointCloud3d;
use crate::point_cloud_material::PointCloudMaterial3d;
use crate::render::point_cloud_uniform::PointCloudUniform;
use bevy_ecs::query::QueryItem;
use bevy_render::extract_component::ExtractComponent;
use bevy_transform::prelude::GlobalTransform;

impl ExtractComponent for PointCloud3d {
    type QueryData = (
        &'static PointCloud3d,
        &'static GlobalTransform,
        &'static PointCloudMaterial3d,
    );
    type QueryFilter = ();
    type Out = (PointCloud3d, PointCloudUniform, PointCloudMaterial3d);

    fn extract_component(
        (point_cloud_3d, global_transform, point_cloud_material_3d): QueryItem<'_, Self::QueryData>,
    ) -> Option<Self::Out> {
        let custom_uniform = PointCloudUniform {
            world_from_local: global_transform.compute_matrix(),
        };
        Some((
            point_cloud_3d.clone(),
            custom_uniform,
            point_cloud_material_3d.clone(),
        ))
    }
}
