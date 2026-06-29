use std::{any::TypeId, collections::BinaryHeap};

use crate::{
    visibility::{
        budget::PointCloudPointBudget, heap_guard::HeapGuard, stack::StackedPointCloudNodeEntity,
    },
    ChildrenMask, GlobalVisiblePointCloudNodes, LoadRequestType, PointCloud, PointCloud3d,
    PointCloudInstances, PointCloudLoadTasks, PointCloudNodeStatus, PointCloudVisibilitySettings,
    PointCloudVisiblityPlugin, ScreenPixelRadiusFilter, SkipPointCloudVisibility,
    VisiblePointCloudEntities,
};
use bevy::{
    asset::Assets,
    camera::{
        primitives::{Aabb, Frustum},
        visibility::{NoAutoAabb, NoFrustumCulling, VisibleEntities},
        Camera, Projection,
    },
    diagnostic::Diagnostics,
    ecs::{
        entity::Entity,
        query::{Changed, Without},
        system::{Commands, Local, Query, Res, ResMut},
    },
    log::warn,
    math::{UVec2, Vec3A},
    platform::{collections::HashMap, time::Instant},
    time::{Real, Time},
    transform::prelude::*,
};

/// Computes and adds an [`Aabb`] component to entities with a
/// [`PoiintCloud3d`] component and without a [`NoFrustumCulling`] component.
pub fn calculate_bounds(
    mut commands: Commands,
    point_clouds: Res<Assets<PointCloud>>,
    new_aabb: Query<
        (Entity, &PointCloud3d),
        (
            Without<Aabb>,
            Without<NoFrustumCulling>,
            Without<NoAutoAabb>,
        ),
    >,
    mut update_aabb: Query<
        (&PointCloud3d, &mut Aabb),
        (
            Changed<PointCloud3d>,
            Without<NoFrustumCulling>,
            Without<NoAutoAabb>,
        ),
    >,
) {
    for (entity, asset_handle) in &new_aabb {
        if let Some(point_cloud) = point_clouds.get(asset_handle)
            && let Some(aabb) = point_cloud.get_root().and_then(|root| root.aabb)
        {
            commands.entity(entity).try_insert(aabb);
        }
    }

    update_aabb
        .par_iter_mut()
        .for_each(|(point_cloud_3d, mut old_aabb)| {
            if let Some(aabb) = point_clouds
                .get(point_cloud_3d)
                .and_then(|point_cloud| point_cloud.get_root().and_then(|root| root.aabb))
            {
                *old_aabb = aabb;
            }
        });
}

pub fn check_point_cloud_nodes_visibility(
    mut diagnostics: Diagnostics,
    _time: Res<Time<Real>>,
    entities: Query<(&PointCloud3d, &GlobalTransform)>,
    // TODO add a way to disable checking of a camera
    mut views: Query<(
        &VisibleEntities,
        &Camera,
        &Frustum,
        &GlobalTransform,
        &Projection,
        &PointCloudVisibilitySettings,
        &mut VisiblePointCloudEntities,
        Option<&SkipPointCloudVisibility>,
    )>,
    point_clouds: Res<Assets<PointCloud>>,
    mut point_cloud_load_tasks: ResMut<PointCloudLoadTasks>,
    mut priority_stack: Local<BinaryHeap<StackedPointCloudNodeEntity>>,
    mut global_visible_point_cloud_nodes: ResMut<GlobalVisiblePointCloudNodes>,
    point_cloud_instances: Res<PointCloudInstances>,
) {
    #[cfg(feature = "trace")]
    let _span = info_span!(
        "check_point_cloud_nodes_visibility",
        name = "check_point_cloud_nodes_visibility"
    )
    .entered();
    let start = Instant::now();
    point_cloud_load_tasks.hierarchy_heap.clear();
    point_cloud_load_tasks.chunk_heap.clear();

    // Clear previous iteration visible point cloud nodes
    global_visible_point_cloud_nodes.clear();

    // for each view
    for (
        visible_entities,
        camera,
        frustum,
        camera_global_transform,
        camera_projection,
        visibility_settings,
        mut visible_point_cloud_entities,
        skip_point_cloud_visibility,
    ) in &mut views
    {
        if !camera.is_active {
            continue;
        }

        if skip_point_cloud_visibility.is_some() {
            visible_point_cloud_entities.changed_this_frame = false;
            continue;
        }

        // Reset previously computed visibility
        visible_point_cloud_entities.clear_all();

        // mark as changed
        visible_point_cloud_entities.changed_this_frame = true;

        let camera_view = CameraView {
            global_transform: camera_global_transform,
            frustum,
            projection: camera_projection,
            physical_target_size: camera.physical_target_size(),
        };

        // get all visible point clouds
        let visible_entities = visible_entities.get(TypeId::of::<PointCloud3d>());

        // create a local scoped priority stack reusing previous allocations
        let mut priority_stack = HeapGuard::new(&mut priority_stack);
        let mut entities_transform = HashMap::new();

        // for each visible point cloud
        for &entity in visible_entities {
            let (component, global_transform) = match entities.get(entity) {
                Ok(item) => item,
                Err(error) => {
                    warn!(
                        "Unable to read point cloud entity for computing nodes visibility: {:#}",
                        error
                    );
                    continue;
                }
            };

            entities_transform.insert(entity, global_transform);

            // get the asset
            let Some(asset) = point_clouds.get(component) else {
                // warn!(
                //     "Asset {} is missing, skip visibility check",
                //     Into::<AssetId<Octree<T>>>::into(component)
                // );
                continue;
            };

            let visible_point_cloud_entity = visible_point_cloud_entities.get_mut(entity);

            // update the asset id
            visible_point_cloud_entity.asset_id = component.into();

            let Some(_) = asset.root else {
                warn!("Point cloud node has not yet hierarchy root loaded.");
                continue;
            };

            let Some(node_root) = asset.get_root() else {
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
                octree: asset,
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
            total_points: 0,
            total_nodes: 0,
        };

        compute_visible_nodes_stack(
            &camera_view,
            &filter,
            &mut budget,
            &mut priority_stack,
            &mut visible_point_cloud_entities,
            &mut global_visible_point_cloud_nodes,
            &entities_transform,
            &mut point_cloud_load_tasks,
        );

        // extract chunk entities for each visible node, if available
        for (entity, point_cloud_entity) in &mut visible_point_cloud_entities.entities {
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
                }
            }
        }

        diagnostics.add_measurement(&PointCloudVisiblityPlugin::BUDGET, || budget.value());
    }

    let duration = start.elapsed();
    let msecs = duration.as_secs_f64() * 1000.0;

    diagnostics.add_measurement(&PointCloudVisiblityPlugin::VISIBILITY_CHECK_TIME, || msecs);
}

#[derive(Clone, Debug)]
pub struct CameraView<'a> {
    pub global_transform: &'a GlobalTransform,
    pub frustum: &'a Frustum,
    pub projection: &'a Projection,
    pub physical_target_size: Option<UVec2>,
}

fn compute_screen_pixel_radius(
    aabb: &Aabb,
    transform: &GlobalTransform,
    camera_view: &CameraView,
) -> Option<f32> {
    #[cfg(feature = "trace")]
    let _span = info_span!("compute_screen_pixel_radius").entered();
    let radius = (aabb.max() - aabb.min()).length() / 2.0;

    // Account for entity scale (uniform or non-uniform) so the filter size stays relative to the
    // transformed point cloud.
    let matrix = transform.affine().matrix3;
    let scale_x = matrix.x_axis.length();
    let scale_y = matrix.y_axis.length();
    let scale_z = matrix.z_axis.length();
    let max_scale = scale_x.max(scale_y).max(scale_z);
    let scaled_radius = radius * max_scale;

    match &camera_view.projection {
        Projection::Perspective(perspective_projection) => {
            let Some(physical_target_size) = &camera_view.physical_target_size else {
                return None;
            };

            let center = transform.affine().transform_point3a(aabb.center);
            let camera_center = Into::<Vec3A>::into(camera_view.global_transform.translation());
            let distance = (center - camera_center).length();

            let slope = (perspective_projection.fov / 2.0).atan();
            let proj_factor = (0.5 * physical_target_size.y as f32) / (slope * distance);

            if distance < scaled_radius {
                return Some(f32::MAX);
            }

            Some(scaled_radius * proj_factor)
        }
        Projection::Orthographic(orthographic_projection) => {
            Some(scaled_radius * orthographic_projection.scale)
        }
        Projection::Custom(_) => None,
    }
}

fn compute_visible_nodes_stack(
    camera_view: &CameraView,
    filter: &ScreenPixelRadiusFilter,
    budget: &mut PointCloudPointBudget,
    stack: &mut BinaryHeap<StackedPointCloudNodeEntity>,
    visible_point_cloud_entities: &mut VisiblePointCloudEntities,
    global_visible_octree_nodes: &mut GlobalVisiblePointCloudNodes,
    entities_transform: &HashMap<Entity, &GlobalTransform>,
    load_tasks: &mut PointCloudLoadTasks,
) {
    #[cfg(feature = "trace")]
    let _span = info_span!("compute_visible_nodes_stack", name = "main").entered();
    while let Some(StackedPointCloudNodeEntity {
        entity,
        asset_id,
        octree: point_cloud,
        node,
        screen_pixel_radius,
        weight,
        mut completely_visible,
        parent_index,
    }) = stack.pop()
    {
        let visible_point_cloud_entity = visible_point_cloud_entities.get_mut(entity);
        let Some(transform) = entities_transform.get(&entity) else {
            warn!("Missing transform for entity {}, skip", entity);
            continue;
        };

        let world_from_local = transform.affine();

        // get the current node future index
        let current_index = visible_point_cloud_entity.node_entities.len();

        #[cfg(feature = "trace")]
        let filter_span = info_span!("compute_visible_nodes_stack", name = "filter").entered();

        // check node visibility
        if !filter.filter(node, transform, camera_view, screen_pixel_radius) {
            continue;
        }

        #[cfg(feature = "trace")]
        drop(filter_span);

        if !completely_visible {
            #[cfg(feature = "trace")]
            let _span = info_span!(
                "compute_visible_nodes_stack",
                name = "check_node_visibility"
            )
            .entered();
            // check if the node aabb against the frustum

            let Some(aabb) = &node.aabb else {
                warn!("Hierarchy node without aabb skipped");
                continue;
            };

            let model_sphere = bevy::camera::primitives::Sphere {
                center: world_from_local.transform_point3a(aabb.center),
                radius: transform.radius_vec3a(aabb.half_extents),
            };

            // Do quick sphere-based frustum culling
            if !camera_view.frustum.intersects_sphere(&model_sphere, false) {
                // this node is not visible, continue
                continue;
            }

            // Check if the aabb is completly inside the frustum
            if camera_view.frustum.contains_aabb(aabb, &world_from_local) {
                // mark as completely visible to prevent later checks
                completely_visible = true;

                // else, do oriented bounding box frustum culling
            } else if !camera_view.frustum.intersects_obb(
                aabb,
                &world_from_local,
                true,
                // we do not set a distance limit because too small might have been already
                // filtered
                false,
            ) {
                // the node is completely outside the frustum, ignore it
                continue;
            }
        }

        match node.chunk.is_some() {
            false => {
                #[cfg(feature = "trace")]
                let _span =
                    info_span!("compute_visible_nodes_stack", name = "hierarchy_only").entered();
                match node.status {
                    PointCloudNodeStatus::Proxy => {
                        load_tasks.queue_load_request(
                            asset_id,
                            node.id,
                            weight,
                            LoadRequestType::Hierarchy,
                        );
                    }
                    PointCloudNodeStatus::Loading => {
                        // the node hierarchy is already loading, nothing to do
                    }
                    PointCloudNodeStatus::Loaded => {
                        load_tasks.queue_load_request(
                            asset_id,
                            node.id,
                            weight,
                            LoadRequestType::Chunk,
                        );
                    }
                }
            }
            // NodeStatus::Loading => {
            //     // the node data is already loading, nothing to do
            // }
            true => {
                #[cfg(feature = "trace")]
                let _span = info_span!("compute_visible_nodes_stack", name = "loaded").entered();
                if budget.add_node(node) {
                    #[cfg(feature = "trace")]
                    let span_iter_children =
                        info_span!("compute_visible_nodes_stack", name = "iter_children").entered();
                    // we have to process child nodes, sending flag `completely_visible` to prevent
                    // useless visibility checks
                    for i in node.children_mask.iter_one_bits() {
                        let child_id = &node.children[i as usize];
                        let Some(child) = point_cloud.get_node(*child_id) else {
                            warn!("missing node in hierarchy, shouldn't happen");
                            continue;
                        };

                        let Some(aabb) = &child.aabb else {
                            continue;
                        };

                        let child_screen_pixel_radius =
                            compute_screen_pixel_radius(aabb, transform, camera_view);
                        let weight = child_screen_pixel_radius.unwrap_or(f32::MAX);

                        #[cfg(feature = "trace")]
                        let span_append_stack =
                            info_span!("compute_visible_nodes_stack", name = "append_stack")
                                .entered();
                        stack.push(StackedPointCloudNodeEntity {
                            entity,
                            asset_id,
                            octree: point_cloud,
                            node: child,
                            screen_pixel_radius: child_screen_pixel_radius,
                            weight: weight.into(),
                            completely_visible,
                            parent_index: Some(current_index),
                        });
                        #[cfg(feature = "trace")]
                        drop(span_append_stack)
                    }
                    #[cfg(feature = "trace")]
                    drop(span_iter_children);

                    // add the current node because it is visible or partially visible
                    let child_index = node.child_index.index();

                    visible_point_cloud_entity.node_entities.push(node.into());
                    global_visible_octree_nodes.add_visible_node(asset_id, node, weight);

                    // if there is a parent, add it to the visible children array
                    if let Some(parent_index) = parent_index {
                        let parent = &mut visible_point_cloud_entity.node_entities[parent_index];

                        parent.children[child_index] = current_index;
                        parent.children_mask |= ChildrenMask::from(node.child_index);
                    }
                }
            }
        }
    }
}
