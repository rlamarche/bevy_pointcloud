use std::collections::BinaryHeap;

use bevy::{
    asset::Assets,
    camera::{
        primitives::CascadesFrusta,
        visibility::{CascadesVisibleEntities, ViewVisibility},
        OrthographicProjection, Projection, ScalingMode,
    },
    ecs::{
        component::Component,
        entity::{Entity, EntityHashMap},
        reflect::ReflectComponent,
        system::{Local, Query, Res, ResMut},
    },
    light::{cascade::Cascade, Cascades, DirectionalLight, DirectionalLightShadowMap},
    log::warn,
    math::{Rect, UVec2, Vec2},
    reflect::{std_traits::ReflectDefault, Reflect},
    transform::components::GlobalTransform,
};
use itertools::izip;

use crate::{
    compute_screen_pixel_radius, compute_visible_nodes_stack,
    visibility::{
        budget::PointCloudPointBudget, heap_guard::HeapGuard, stack::StackedPointCloudNodeEntity,
    },
    CameraView, GlobalVisiblePointCloudChunks, GlobalVisiblePointCloudNodes, PointCloud,
    PointCloud3d, PointCloudChunk3d, PointCloudInstances, PointCloudLoadTasks, PointCloudTopology,
    PointCloudVisibilitySettings, ScreenPixelRadiusFilter, SkipPointCloudVisibility,
    VisiblePointCloudFlatEntities, VisiblePointCloudOctreeEntities,
};

#[derive(Component, Clone, Debug, Default, Reflect)]
#[reflect(Component, Default, Clone)]
pub struct CascadesVisiblePointCloudEntities {
    /// Map of view entity to the visible point cloud entities for each cascade frustum.
    #[reflect(ignore, clone)]
    pub entities: EntityHashMap<
        Vec<(
            VisiblePointCloudFlatEntities,
            VisiblePointCloudOctreeEntities,
        )>,
    >,
}

impl CascadesVisiblePointCloudEntities {
    pub fn get_mut(
        &mut self,
        entity: Entity,
    ) -> &mut Vec<(
        VisiblePointCloudFlatEntities,
        VisiblePointCloudOctreeEntities,
    )> {
        self.entities.entry(entity).or_default()
    }

    pub fn clear_all(&mut self) {
        // Don't just nuke the hash table; we want to reuse allocations.
        for cascade_point_cloud_entities in self.entities.values_mut() {
            for (visible_point_cloud_flat_entities, visible_point_cloud_octree_entities) in
                cascade_point_cloud_entities
            {
                visible_point_cloud_flat_entities.clear();
                for point_cloud_entities in
                    visible_point_cloud_octree_entities.entities.values_mut()
                {
                    point_cloud_entities.asset_id = Default::default();
                    point_cloud_entities.node_entities.clear();
                }
            }
        }
    }
}

pub fn check_point_cloud_nodes_dir_lights_visibility(
    entities: Query<(&PointCloud3d, &GlobalTransform, Option<&PointCloudChunk3d>)>,
    // TODO add a way to disable checking of a camera
    mut lights: Query<(
        &DirectionalLight,
        &Cascades,
        &CascadesFrusta,
        &CascadesVisibleEntities,
        &GlobalTransform,
        &ViewVisibility,
        &PointCloudVisibilitySettings,
        &mut CascadesVisiblePointCloudEntities,
        Option<&SkipPointCloudVisibility>,
    )>,
    point_clouds: Res<Assets<PointCloud>>,
    mut point_cloud_load_tasks: ResMut<PointCloudLoadTasks>,
    mut priority_stack: Local<BinaryHeap<StackedPointCloudNodeEntity>>,
    mut global_visible_point_cloud_nodes: ResMut<GlobalVisiblePointCloudNodes>,
    mut global_visible_point_cloud_chunks: ResMut<GlobalVisiblePointCloudChunks>,
    point_cloud_instances: Res<PointCloudInstances>,
    shadow_map_config: Res<DirectionalLightShadowMap>,
) {
    point_cloud_load_tasks.hierarchy_heap.clear();
    point_cloud_load_tasks.chunk_heap.clear();

    // Clear previous iteration visible point cloud nodes
    global_visible_point_cloud_nodes.clear();
    global_visible_point_cloud_chunks.clear();

    // for each view
    for (
        _dir_light,
        cascades,
        frusta,
        cascades_visible_entities,
        light_global_transform,
        view_visibility,
        visibility_settings,
        mut cascade_visible_point_cloud_entities,
        skip_point_cloud_visibility,
    ) in &mut lights
    {
        if !view_visibility.get() {
            continue;
        }

        if (skip_point_cloud_visibility).is_some() {
            for (_, visible_point_cloud_entities) in
                &mut cascade_visible_point_cloud_entities.entities
            {
                for (visible_point_cloud_flat_entity, visible_point_cloud_octree_entity) in
                    visible_point_cloud_entities
                {
                    visible_point_cloud_flat_entity.changed_this_frame = false;
                    visible_point_cloud_octree_entity.changed_this_frame = false;
                }
            }
            continue;
        }

        // Reset previously computed visibility
        cascade_visible_point_cloud_entities.clear_all();

        // Prepare `cascade_visible_point_cloud_entities`
        let mut views_to_remove = Vec::new();

        for (view, cascade_view_entities) in &mut cascade_visible_point_cloud_entities.entities {
            match frusta.frusta.get(view) {
                Some(view_frusta) => {
                    cascade_view_entities.resize(view_frusta.len(), Default::default());
                }
                None => views_to_remove.push(*view),
            };
        }

        for (view, frusta) in &frusta.frusta {
            cascade_visible_point_cloud_entities
                .entities
                .entry(*view)
                .or_insert_with(|| vec![Default::default(); frusta.len()]);
        }

        for v in views_to_remove {
            cascade_visible_point_cloud_entities.entities.remove(&v);
        }

        for (view, view_frusta) in &frusta.frusta {
            let Some(view_cascades) = cascades.cascades.get(view) else {
                warn!("Missing cascades for view {:?}", view);
                continue;
            };
            let Some(view_visible_entities) = cascades_visible_entities.entities.get(view) else {
                warn!("Missing cascades visible entities for view {:?}", view);
                continue;
            };
            let view_cascade_visible_point_cloud_entities =
                cascade_visible_point_cloud_entities.get_mut(*view);

            // TODO: parallelize
            for (
                frustum,
                frustum_visible_entities,
                view_cascade,
                (visible_point_cloud_flat_entities, visible_point_cloud_octree_entities),
            ) in izip!(
                view_frusta,
                view_visible_entities,
                view_cascades,
                view_cascade_visible_point_cloud_entities,
            ) {
                // mark as changed
                visible_point_cloud_flat_entities.changed_this_frame = true;
                visible_point_cloud_octree_entities.changed_this_frame = true;

                // Compute the light projection.
                // It's an orthographic projection because on directionnal lights, all rays are
                // parallels.
                let projection =
                    Projection::Orthographic(orthographic_projection_from_cascade(view_cascade));

                let camera_view = CameraView {
                    global_transform: light_global_transform,
                    frustum,
                    projection: &projection,
                    physical_target_size: Some(UVec2::splat(shadow_map_config.size as u32)),
                };

                // create a local scoped priority stack reusing previous allocations
                let mut priority_stack = HeapGuard::new(&mut priority_stack);
                let mut entities_transform = EntityHashMap::new();

                // for each visible point cloud
                for &entity in &frustum_visible_entities.entities {
                    let Ok((component, global_transform, maybe_chunk)) = entities.get(entity)
                    else {
                        // this is ignorable because `frustum_visible_entities` contains also non
                        // point cloud entities.
                        continue;
                    };

                    entities_transform.insert(entity, global_transform);

                    // get the asset
                    let Some(asset) = point_clouds.get(component) else {
                        continue;
                    };

                    let octree = match &asset.topology {
                        PointCloudTopology::Empty => {
                            continue;
                        }
                        // if this is a flat point cloud, just add it to the visible chunks if
                        // loaded
                        PointCloudTopology::Flat(_) => {
                            if let Some(_) = maybe_chunk {
                                global_visible_point_cloud_chunks.add_visible_chunk(
                                    entity,
                                    entity,
                                    0.0.into(),
                                );
                                visible_point_cloud_flat_entities
                                    .entities
                                    .insert(entity, component.into());
                            }
                            continue;
                        }
                        PointCloudTopology::Octree(octree) => octree,
                    };

                    let visible_point_cloud_entity =
                        visible_point_cloud_octree_entities.get_mut(entity);

                    // update the asset id
                    visible_point_cloud_entity.asset_id = component.into();

                    let Some(node_root) = octree.get_root() else {
                        warn!("Point cloud node has not yet hierarchy root loaded.");
                        continue;
                    };

                    let Some(aabb) = &node_root.aabb else {
                        warn!("A point cloud root node is missing Aabb");
                        continue;
                    };

                    let screen_pixel_radius =
                        compute_screen_pixel_radius(aabb, global_transform, &camera_view);

                    // add the root octree to the priority stack
                    priority_stack.push(StackedPointCloudNodeEntity {
                        entity,
                        asset_id: visible_point_cloud_entity.asset_id,
                        octree,
                        node: node_root,
                        weight: screen_pixel_radius.unwrap_or(f32::MAX).into(),
                        screen_pixel_radius,
                        // TODO check for this ?
                        completely_visible: false,
                        parent_index: None,
                    });
                }

                let filter = ScreenPixelRadiusFilter {
                    min_radius: visibility_settings.min_radius,
                };

                let mut budget = PointCloudPointBudget {
                    point_budget: visibility_settings.point_budget,
                    max_depth: visibility_settings.max_depth,
                    total_points: 0,
                    total_nodes: 0,
                };

                compute_visible_nodes_stack(
                    &camera_view,
                    &filter,
                    &mut budget,
                    &mut priority_stack,
                    visible_point_cloud_octree_entities,
                    &entities_transform,
                    &mut point_cloud_load_tasks,
                    false,
                );

                // info!(
                //     "Visible point cloud entities: {:#?}",
                //     visible_point_cloud_entities
                //         .entities
                //         .iter()
                //         .fold(0, |count, (_, entities)| {
                //             count + entities.node_entities.len()
                //         })
                // );

                // extract chunk entities for each visible node, if available and populate resource
                // [`GlobalVisiblePointCloudChunks`].
                for (entity, point_cloud_entity) in
                    &mut visible_point_cloud_octree_entities.entities
                {
                    let Some(point_cloud_instance) =
                        point_cloud_instances.get(&point_cloud_entity.asset_id)
                    else {
                        continue;
                    };

                    for node_entity in &mut point_cloud_entity.node_entities {
                        let Some(chunk_instance) = point_cloud_instance.get(entity) else {
                            continue;
                        };
                        if let Some(chunk_id) = node_entity.chunk_id
                            && let Some(&chunk_entity) = chunk_instance.get(&chunk_id)
                        {
                            node_entity.entity = Some(chunk_entity);
                            global_visible_point_cloud_chunks.add_visible_chunk(
                                *entity,
                                chunk_entity,
                                node_entity.weight.into(),
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Extract the orthographic projection from a cascade to check octree nodes visibility.
pub fn extract_orthographic_projection(cascade: &Cascade) -> OrthographicProjection {
    // 1. Extraire le diamètre de la cascade depuis clip_from_cascade.x_axis.x
    // Puisque x_axis.x = 2.0 / cascade_diameter, alors :
    let cascade_diameter = 2.0 / cascade.clip_from_cascade.x_axis.x;

    // 2. Calculer l'area (la boîte est centrée et carrée)
    let half_size = cascade_diameter / 2.0;
    let area = Rect {
        min: Vec2::new(-half_size, -half_size),
        max: Vec2::new(half_size, half_size),
    };

    // 3. Extraire la profondeur (far) depuis le paramètre 'r' dans clip_from_cascade.z_axis.z
    // Puisque z_axis.z = r = 1.0 / depth, alors :
    let depth = 1.0 / cascade.clip_from_cascade.z_axis.z;

    // En Reverse-Z (utilisé ici), le plan near local est à 0.0 et le far à 'depth'
    let near = 0.0;
    let far = depth;

    OrthographicProjection {
        near,
        far,
        viewport_origin: Vec2::new(0.5, 0.5), // Le centre est à (0,0) dans l'espace de la cascade
        scaling_mode: ScalingMode::Fixed {
            width: cascade_diameter,
            height: cascade_diameter,
        },
        scale: 1.0,
        area,
    }
}

// /// Reconstructs an `OrthographicProjection` purely from the data stored in a `Cascade`.
// ///
// /// This works by algebraically inverting the matrix layout that Bevy itself uses to build
// /// `clip_from_cascade` (and, identically, `OrthographicProjection::get_clip_from_view`):
// ///
// /// ```text
// /// Mat4::orthographic_rh(left, right, bottom, top, far, near) // note: near/far swapped
// (reverse-Z) /// ```
// ///
// /// which expands to:
// ///   x_axis.x = 2 / (right - left)
// ///   y_axis.y = 2 / (top - bottom)
// ///   w_axis.x = -(right + left) / (right - left)
// ///   w_axis.y = -(top + bottom) / (top - bottom)
// ///   z_axis.z = 1 / (far - near)
// ///   w_axis.z = far / (far - near)
// ///
// /// Solving these for left/right/bottom/top/near/far gives us back the exact frustum
// /// that produced the matrix (up to floating point rounding).
// pub fn orthographic_projection_from_cascade(cascade: &Cascade) -> OrthographicProjection {
//     let m = cascade.clip_from_cascade;

//     // --- Horizontal extent (left/right) ---
//     let width = 2.0 / m.x_axis.x;
//     let sum_x = -m.w_axis.x * width; // right + left
//     let right = (sum_x + width) * 0.5;
//     let left = right - width;

//     // --- Vertical extent (bottom/top) ---
//     let height = 2.0 / m.y_axis.y;
//     let sum_y = -m.w_axis.y * height; // top + bottom
//     let top = (sum_y + height) * 0.5;
//     let bottom = top - height;

//     // --- Depth extent (near/far), accounting for Bevy's reverse-Z swap ---
//     let inv_range = m.z_axis.z; // = 1 / (far - near)
//     let far = m.w_axis.z / inv_range;
//     let near = far - 1.0 / inv_range;

//     OrthographicProjection {
//         near,
//         far,
//         // Irrelevant here: `viewport_origin`/`scaling_mode` only matter when Bevy's camera
//         // system calls `update()` (e.g. on window resize). Since we set `area` directly and
//         // this projection isn't driven by a live Camera, they're never used to rebuild `area`.
//         viewport_origin: Vec2::new(0.5, 0.5),
//         scaling_mode: ScalingMode::Fixed { width, height },
//         scale: 1.0,
//         area: Rect {
//             min: Vec2::new(left, bottom),
//             max: Vec2::new(right, top),
//         },
//     }
// }

/// Reconstructs an `OrthographicProjection` from a `Cascade`, exploiting the fact that
/// `calculate_cascade()` (in `bevy_light`) always produces a projection that is:
///   - symmetric in X/Y (left = -right, bottom = -top), since `cascade_from_world` already
///     re-centers the cascade around `near_plane_center`.
///   - has `near = 0.0` exactly, since the cascade's local origin *is* the near plane
///     (`near_plane_center.z = max.z`).
///   - has `w_axis.z` hard-coded to `1.0` (not computed), so `far` is recovered with a single
///     division.
///
/// This means we don't need to solve the general orthographic-matrix-inversion system;
/// two divisions on `clip_from_cascade` are enough.
///
/// NOTE: this relies on Bevy's current `calculate_cascade()` implementation. If that
/// function's matrix layout ever changes, this needs to be revisited.
pub fn orthographic_projection_from_cascade(cascade: &Cascade) -> OrthographicProjection {
    let m = cascade.clip_from_cascade;

    // x_axis.x == 2.0 / cascade_diameter  =>  half_extent == 1.0 / x_axis.x
    let half_extent = 1.0 / m.x_axis.x; // same value on X and Y, cascade is always square

    // z_axis.z == r == 1.0 / (max.z - min.z), and near is always 0, so far == 1.0 / z_axis.z
    let far = 1.0 / m.z_axis.z;
    let near = 0.0;

    OrthographicProjection {
        near,
        far,
        viewport_origin: Vec2::new(0.5, 0.5), // unused, we set `area` directly
        scaling_mode: ScalingMode::Fixed {
            width: half_extent * 2.0,
            height: half_extent * 2.0,
        },
        scale: 1.0,
        area: Rect {
            min: Vec2::splat(-half_extent),
            max: Vec2::splat(half_extent),
        },
    }
}
