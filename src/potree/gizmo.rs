use super::asset::PotreePointCloud;
use crate::potree::hierarchy::HierarchySnapshot;
use crate::potree::point_cloud::PotreeMainCamera;
use bevy_asset::prelude::*;
use bevy_camera::primitives::{Aabb, Frustum};
use bevy_camera::Camera;
use bevy_color::palettes::basic::RED;
use bevy_color::Srgba;
use bevy_derive::Deref;
use bevy_ecs::prelude::*;
use bevy_gizmos::prelude::*;
use bevy_math::prelude::*;
use bevy_mesh::Mesh3d;
use bevy_pbr::{MeshMaterial3d, StandardMaterial};
use bevy_render::alpha::AlphaMode;
use bevy_rich_text3d::{Text3d, Text3dStyling, TextAtlas};
use bevy_transform::prelude::*;
use potree::octree::node::iter_one_bits;
use potree::prelude::OctreeNodeSnapshot;
use std::num::NonZero;

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

#[derive(Component, Deref)]
pub struct MyMaterial(pub Handle<StandardMaterial>);

#[derive(Component)]
pub struct NodeLabel;

pub fn init_text_gizmos(mut materials: ResMut<Assets<StandardMaterial>>, mut commands: Commands) {
    let text_material = materials.add(StandardMaterial {
        base_color_texture: Some(TextAtlas::DEFAULT_IMAGE.clone()),
        alpha_mode: AlphaMode::Mask(0.5),
        unlit: true,
        cull_mode: None,
        ..Default::default()
    });

    commands.spawn(MyMaterial(text_material.clone()));
}

pub fn update_text_gizmos(
    text_material: Query<&MyMaterial>,
    node_labels: Query<Entity, With<NodeLabel>>,
    mut commands: Commands,
    potree_point_clouds_3d: Query<
        (Entity, &GlobalTransform, Option<&HierarchySnapshot>),
        With<DrawPotreeGizmo>,
    >,
    camera: Query<(&Camera, &Transform), With<PotreeMainCamera>>,
) {
    let (_camera, camera_transform) = camera.single().unwrap();
    let camera_pos: Vec3 = camera_transform.compute_affine().translation.into();

    let text_material = text_material
        .single()
        .expect("Text material is missing")
        .0
        .clone();

    for entity in node_labels.iter() {
        commands.entity(entity).despawn();
    }

    for (entity, global_transform, hierarchy_snapshot) in potree_point_clouds_3d {
        let Some(HierarchySnapshot(visible_nodes)) = hierarchy_snapshot else {
            // warn!("No hierarchy snapshot available.");
            continue;
        };

        // Extract parent rotation (to cancel it later)
        let (_, parent_rot, parent_pos) = global_transform.to_scale_rotation_translation();

        for node in visible_nodes {
            let center = (node.bounding_box.min + node.bounding_box.max) / 2.0;

            // --- 1) offset local dans le repère du cube ---
            let center_local = (node.bounding_box.min + node.bounding_box.max) / 2.0;
            let offset_local = center_local.as_vec3();

            // --- 2) calcul de la position globale du label ---
            let label_global_pos = parent_pos + parent_rot * offset_local;

            // --- 3) direction vers la caméra ---
            let to_cam = (camera_pos - label_global_pos).normalize();

            // billboard "horizontal" (texte reste vertical)
            let forward = Vec3::new(to_cam.x, 0.0, to_cam.z).normalize();
            let billboard_rot = Quat::from_rotation_arc(Vec3::Z, forward);

            // --- 4) rotation locale = inverse(parent) * billboard ---
            let local_rotation = parent_rot.inverse() * billboard_rot;

            let spawned_text = commands
                .spawn((
                    NodeLabel,
                    Text3d::new(node.name.clone()),
                    Mesh3d::default(),
                    MeshMaterial3d(text_material.clone()),
                    Text3dStyling {
                        size: 32.,
                        stroke: NonZero::new(10),
                        color: Srgba::new(1., 0., 0., 1.),
                        stroke_color: Srgba::BLACK,
                        world_scale: Some(Vec2::splat(1.0)),
                        layer_offset: 0.001,
                        ..Default::default()
                    },
                    // Transform::from_translation(center.as_vec3())
                    Transform {
                        translation: offset_local, // reste local
                        rotation: local_rotation,
                        scale: Vec3::splat(1.0_f32 / 2.0_f32.powf(node.level as f32)),
                        // billboard calculé
                        ..Default::default()
                    },
                ))
                .id();

            commands.entity(entity).add_child(spawned_text);
        }
    }
}

pub fn update_gizmos(
    potree_point_clouds: Res<Assets<PotreePointCloud>>,
    mut gizmo_assets: ResMut<Assets<GizmoAsset>>,
    potree_point_clouds_3d: Query<
        (Entity, &Gizmo, &GlobalTransform, Option<&HierarchySnapshot>),
        With<DrawPotreeGizmo>,
    >,
    mut commands: Commands,
) {
    for (entity, gizmo, global_transform, hierarchy_snapshot) in potree_point_clouds_3d {
        let Some(gizmo_asset) = gizmo_assets.get_mut(&gizmo.handle) else {
            continue;
        };

        let Some(HierarchySnapshot(visible_nodes)) = hierarchy_snapshot else {
            // warn!("No hierarchy snapshot available.");
            continue;
        };

        // let visible_nodes = compute_visible_nodes(
        //     visible_nodes,
        //     &hierarchy_snapshot.0[0],
        //     global_transform,
        //     frustum,
        // );

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
    nodes: &'a Vec<OctreeNodeSnapshot>,
    node: &'a OctreeNodeSnapshot,
    transform: &GlobalTransform,
    frustum: &Frustum,
) -> Vec<&'a OctreeNodeSnapshot> {
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
    iter_one_bits(node.children_mask)
        .into_iter()
        .flat_map(|child_index| {
            compute_visible_nodes(
                nodes,
                &nodes[node.children[child_index]],
                transform,
                frustum,
            )
        })
        .collect::<Vec<_>>()
}
