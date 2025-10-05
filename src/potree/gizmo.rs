use super::asset::PotreePointCloud;
use super::point_cloud::{PotreeMainCamera, PotreePointCloud3d};
use crate::potree::hierarchy::HierarchySnapshot;
use bevy_asset::prelude::*;
use bevy_camera::primitives::{Aabb, Frustum};
use bevy_color::palettes::basic::RED;
use bevy_ecs::prelude::*;
use bevy_gizmos::prelude::*;
use bevy_log::prelude::*;
use bevy_math::prelude::*;
use bevy_tasks::{block_on, futures_lite::future};
use bevy_transform::prelude::*;

#[derive(Component)]
pub struct DrawPotreeGizmo;

pub fn init_gizmos(
    mut commands: Commands,
    mut gizmo_assets: ResMut<Assets<GizmoAsset>>,
    potree_point_clouds_3d: Query<Entity, (With<DrawPotreeGizmo>, Without<Gizmo>)>,
) {
    for entity in potree_point_clouds_3d {
        let gizmo = GizmoAsset::new();
        let handle = gizmo_assets.add(gizmo);

        commands.entity(entity).insert(Gizmo {
            handle,
            line_config: GizmoLineConfig {
                width: 1.,
                ..Default::default()
            },
            ..Default::default()
        });
    }
}

pub fn update_gizmos(
    frustum: Query<&Frustum, With<PotreeMainCamera>>,
    potree_point_clouds: Res<Assets<PotreePointCloud>>,
    mut gizmo_assets: ResMut<Assets<GizmoAsset>>,
    potree_point_clouds_3d: Query<
        (
            Entity,
            &PotreePointCloud3d,
            &Gizmo,
            &GlobalTransform,
            Option<&HierarchySnapshot>,
        ),
        With<DrawPotreeGizmo>,
    >,
) {
    let Ok(frustum) = frustum.single() else {
        return;
    };

    for (entity, potree_point_cloud_3d, gizmo, global_transform, hierarchy_snapshot) in
        potree_point_clouds_3d
    {
        let Some(potree_point_cloud) = potree_point_clouds.get(&potree_point_cloud_3d.handle)
        else {
            continue;
        };
        let Some(gizmo_asset) = gizmo_assets.get_mut(&gizmo.handle) else {
            continue;
        };

        let Some(hierarchy_snapshot) = hierarchy_snapshot else {
            // warn!("No hierarchy snapshot available.");
            continue;
        };

        let visible_nodes = compute_visible_nodes(&hierarchy_snapshot.0, &global_transform, frustum);

        gizmo_asset.clear();
        for node in visible_nodes {
            let center = (node.bounding_box.min + node.bounding_box.max) / 2.0;
            let scale = node.bounding_box.max - node.bounding_box.min;
            gizmo_asset.cuboid(
                Transform::from_translation(Vec3::new(
                    center.x as f32,
                    center.y as f32,
                    center.z as f32,
                ))
                .with_scale(Vec3::new(
                    scale.x as f32,
                    scale.y as f32,
                    scale.z as f32,
                )),
                RED,
            );
        }
    }
}

fn compute_visible_nodes<'a>(
    node: &'a potree::octree::snapshot::OctreeNodeSnapshot,
    transform: &GlobalTransform,
    frustum: &Frustum,
) -> Vec<&'a potree::octree::snapshot::OctreeNodeSnapshot> {
    let min = node.bounding_box.min;
    let max = node.bounding_box.max;

    let model_aabb = Aabb::from_min_max(
        Vec3::new(min.x as f32, min.y as f32, min.z as f32),
        Vec3::new(max.x as f32, max.y as f32, max.z as f32),
    );

    let world_from_local = transform.affine();

    let model_sphere = bevy_camera::primitives::Sphere {
        center: world_from_local.transform_point3a(model_aabb.center),
        radius: transform.radius_vec3a(model_aabb.half_extents),
    };

    // Do quick sphere-based frustum culling
    if !frustum.intersects_sphere(&model_sphere, false) {
        return vec![];
    }

    if (frustum.contains_aabb(&model_aabb, &world_from_local)) {
        // if it is completely contained, return all nodes recursively
        return vec![node];
        // return std::iter::once(node).chain(node.iter()).collect();
    }

    // Do aabb-based frustum culling
    if !frustum.intersects_obb(&model_aabb, &world_from_local, true, false) {
        return vec![];
    }

    // We intersect this node, recursively check children for visibility
    if node.children.len() > 0 {
        node.children
            .iter()
            .flat_map(|child| compute_visible_nodes(child, transform, frustum))
            .collect::<Vec<_>>()
    } else {
        // if no children, return the node itself
        vec![node]
    }
}
