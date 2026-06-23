use bevy_asset::Assets;
use bevy_camera::primitives::Aabb;
use bevy_ecs::prelude::*;

use crate::{point::Point, point_cloud::PointCloud, render::PointCloud3d};

/// Compute AABB for point clouds
///
/// # Arguments
///
/// * `point_clouds_without_aabb`:
/// * `point_clouds`:
/// * `commands`:
///
/// returns: ()
///
/// # Examples
///
/// ```
/// ```
#[allow(clippy::type_complexity)]
pub fn compute_point_cloud_aabb<T: Point>(
    point_clouds_without_aabb: Query<
        (Entity, &PointCloud3d<T>),
        (With<PointCloud3d<T>>, Without<Aabb>),
    >,
    point_clouds: Res<Assets<PointCloud<T>>>,
    mut commands: Commands,
) {
    for (entity, point_cloud_3d) in point_clouds_without_aabb.iter() {
        let Some(point_cloud) = point_clouds.get(point_cloud_3d) else {
            continue;
        };

        // use pre computed aabb if available
        if let Some(aabb) = match &point_cloud.aabb {
            Some(aabb) => Some(*aabb),
            None => Aabb::enclosing(point_cloud.points.iter().map(|p| p.position())),
        } {
            commands.entity(entity).insert(aabb);
        }
    }
}
