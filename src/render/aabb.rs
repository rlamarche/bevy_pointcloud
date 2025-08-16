use crate::point_cloud::PointCloud;
use crate::render::PointCloud3d;
use bevy_asset::Assets;
use bevy_ecs::prelude::*;
use bevy_render::primitives::Aabb;
use bevy_render::view::NoFrustumCulling;

#[derive(Component)]
pub struct AabbComputed;

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
///
/// ```
pub fn compute_point_cloud_aabb(
    point_clouds_without_aabb: Query<
        (Entity, &PointCloud3d),
        (With<PointCloud3d>, Without<NoFrustumCulling>, Without<AabbComputed>),
    >,
    point_clouds: Res<Assets<PointCloud>>,
    mut commands: Commands,
) {
    for (entity, point_cloud_3d) in point_clouds_without_aabb.iter() {
        let Some(point_cloud) = point_clouds.get(point_cloud_3d) else {
            continue;
        };

        let Some(aabb) = Aabb::enclosing(point_cloud.points.iter().map(|p| p.position)) else {
            continue;
        };

        commands.entity(entity).insert((aabb, AabbComputed));
    }
}
