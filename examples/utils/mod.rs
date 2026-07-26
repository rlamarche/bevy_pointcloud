use bevy::{color::palettes::css::RED, prelude::*};
use bevy_pointcloud::{prelude::*, VisiblePointCloudEntities};

pub fn draw_gizmos(
    point_clouds: Res<Assets<PointCloud>>,
    entities: Query<&GlobalTransform, With<PointCloud3d>>,
    visible_point_cloud_entities: Query<&VisiblePointCloudEntities>,
    mut gizmos: Gizmos,
) {
    // for each view
    for visible_point_cloud_entities in visible_point_cloud_entities {
        // for each visible point cloud in this view
        for (entity, visible_point_cloud_entity) in &visible_point_cloud_entities.entities {
            let Ok(global_transform) = entities.get(*entity) else {
                continue;
            };

            let Some(point_cloud) = point_clouds.get(visible_point_cloud_entity.asset_id) else {
                continue;
            };

            // for each visible node in this view
            for visible_node in &visible_point_cloud_entity.node_entities {
                let Some(node) = point_cloud.get_node(visible_node.id) else {
                    continue;
                };

                if let Some(aabb) = &node.aabb {
                    let center = aabb.center;
                    let scale = aabb.half_extents * 2.0;

                    let local_transform =
                        Transform::from_translation(center.into()).with_scale(scale.into());

                    let world_transform = global_transform.mul_transform(local_transform);

                    gizmos.cube(world_transform, RED);
                }
            }
        }
    }
}
