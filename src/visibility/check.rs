use std::{any::TypeId, collections::BinaryHeap};

use crate::{
    visibility::{
        budget::PointCloudPointBudget, heap_guard::HeapGuard, stack::StackedPointCloudNodeEntity,
    },
    CascadesVisiblePointCloudEntities, ChildrenMask, GlobalVisiblePointCloudChunks,
    GlobalVisiblePointCloudNodes, LoadRequestType, PointCloud, PointCloud3d, PointCloudChunk3d,
    PointCloudInstances, PointCloudLoadTasks, PointCloudNodeStatus, PointCloudTopology,
    PointCloudVisibilitySettings, PointCloudVisiblityPlugin, ScreenPixelRadiusFilter,
    SkipPointCloudVisibility, VisiblePointCloudFlatEntities, VisiblePointCloudNodeEntity,
    VisiblePointCloudOctreeEntities,
};
use bevy::{
    asset::Assets,
    camera::{
        primitives::{Aabb, Frustum},
        visibility::{SetViewVisibility, ViewVisibility, VisibleEntities},
        Camera, Projection,
    },
    diagnostic::Diagnostics,
    ecs::{
        entity::EntityHashMap,
        lifecycle::RemovedComponents,
        query::With,
        system::{Local, Query, Res, ResMut},
    },
    log::warn,
    math::{UVec2, Vec3A},
    platform::time::Instant,
    time::{Real, Time},
    transform::prelude::*,
};

pub fn check_point_cloud_nodes_visibility(
    mut diagnostics: Diagnostics,
    _time: Res<Time<Real>>,
    entities: Query<(&PointCloud3d, &GlobalTransform, Option<&PointCloudChunk3d>)>,
    // TODO add a way to disable checking of a camera
    mut views: Query<(
        &VisibleEntities,
        &Camera,
        &Frustum,
        &GlobalTransform,
        &Projection,
        &PointCloudVisibilitySettings,
        &mut VisiblePointCloudOctreeEntities,
        &mut VisiblePointCloudFlatEntities,
        Option<&SkipPointCloudVisibility>,
    )>,
    point_clouds: Res<Assets<PointCloud>>,
    mut point_cloud_load_tasks: ResMut<PointCloudLoadTasks>,
    mut priority_stack: Local<BinaryHeap<StackedPointCloudNodeEntity>>,
    mut global_visible_point_cloud_nodes: ResMut<GlobalVisiblePointCloudNodes>,
    mut global_visible_point_cloud_chunks: ResMut<GlobalVisiblePointCloudChunks>,
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
    global_visible_point_cloud_chunks.clear();

    // for each view
    for (
        visible_entities,
        camera,
        frustum,
        camera_global_transform,
        camera_projection,
        visibility_settings,
        mut visible_point_cloud_octree_entities,
        mut visible_point_cloud_flat_entities,
        skip_point_cloud_visibility,
    ) in &mut views
    {
        if !camera.is_active {
            continue;
        }

        if skip_point_cloud_visibility.is_some() {
            visible_point_cloud_octree_entities.changed_this_frame = false;
            visible_point_cloud_flat_entities.changed_this_frame = false;
            continue;
        }

        // Reset previously computed visibility
        visible_point_cloud_octree_entities.clear_all();
        visible_point_cloud_flat_entities.clear();

        // mark as changed
        visible_point_cloud_octree_entities.changed_this_frame = true;
        visible_point_cloud_flat_entities.changed_this_frame = true;

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
        let mut entities_transform = EntityHashMap::new();

        // for each visible point cloud
        for &entity in visible_entities {
            let Ok((component, global_transform, maybe_chunk)) = entities.get(entity) else {
                warn!("Unable to read point cloud entity for computing nodes visibility");
                continue;
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

            let octree = match &asset.topology {
                PointCloudTopology::Empty => {
                    continue;
                }
                // if this is a flat point cloud, just add it to the visible chunks if loaded
                PointCloudTopology::Flat(_) => {
                    if maybe_chunk.is_some() {
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

            let visible_point_cloud_entity = visible_point_cloud_octree_entities.get_mut(entity);

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
            &mut visible_point_cloud_octree_entities,
            &entities_transform,
            &mut point_cloud_load_tasks,
            false,
        );

        // extract chunk entities for each visible node, if available and populate resource
        // [`GlobalVisiblePointCloudChunks`].
        // It also fills the [`VisiblePointCloudNodeEntity::entity`] field (not done during the
        // visiblity check to reduce lookups).
        for (entity, point_cloud_entity) in &mut visible_point_cloud_octree_entities.entities {
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

// TODO: cache the scale for each node to prevent calculating max scale for each nodes
pub fn compute_screen_pixel_radius(
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

            let slope = (perspective_projection.fov / 2.0).tan();
            let proj_factor = (0.5 * physical_target_size.y as f32) / (slope * distance);

            if distance < scaled_radius {
                return Some(f32::MAX);
            }

            Some(scaled_radius * proj_factor)
        }
        Projection::Orthographic(orthographic_projection) => {
            let Some(physical_target_size) = &camera_view.physical_target_size else {
                return None;
            };

            let world_height =
                orthographic_projection.area.height() * orthographic_projection.scale;

            if world_height <= 0.0 {
                return None;
            }

            let pixels_per_world_unit = physical_target_size.y as f32 / world_height;

            Some(scaled_radius * pixels_per_world_unit)
        }
        Projection::Custom(_) => None,
    }
}

pub fn compute_visible_nodes_stack(
    camera_view: &CameraView,
    filter: &ScreenPixelRadiusFilter,
    budget: &mut PointCloudPointBudget,
    stack: &mut BinaryHeap<StackedPointCloudNodeEntity>,
    visible_point_cloud_entities: &mut VisiblePointCloudOctreeEntities,
    entities_transform: &EntityHashMap<&GlobalTransform>,
    load_tasks: &mut PointCloudLoadTasks,
    eager_stop: bool,
) {
    #[cfg(feature = "trace")]
    let _span = info_span!("compute_visible_nodes_stack", name = "main").entered();
    while let Some(StackedPointCloudNodeEntity {
        entity,
        asset_id,
        octree,
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

        // Check node visibility.
        // we ignore root node to prevent it from disappearing when getting far away.
        if filter.filter(node, transform, camera_view, screen_pixel_radius) {
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

        let budget_check = budget.check(node);

        if eager_stop && !budget_check {
            // skip checking for visibility if the budget is reached and eager stop is asked
            continue;
        }

        // check if the chunk is loaded
        match node.chunk.is_some() {
            false => {
                #[cfg(feature = "trace")]
                let _span =
                    info_span!("compute_visible_nodes_stack", name = "hierarchy_only").entered();
                match node.status {
                    PointCloudNodeStatus::Proxy => {
                        // always load sub hierarchy
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
                        // load the chunk only if w're going to display it
                        if budget_check {
                            load_tasks.queue_load_request(
                                asset_id,
                                node.id,
                                weight,
                                LoadRequestType::Chunk,
                            );
                        }
                    }
                }
            }
            true => {
                #[cfg(feature = "trace")]
                let _span = info_span!("compute_visible_nodes_stack", name = "loaded").entered();

                if budget_check {
                    // take into account the node
                    budget.add_node(node);

                    // add the current node to the visible nodes list
                    visible_point_cloud_entity.node_entities.push(
                        VisiblePointCloudNodeEntity::from_with_weight(node, weight.into()),
                    );

                    // if there is a parent, add it to the visible children array
                    if let Some(parent_index) = parent_index {
                        let parent = &mut visible_point_cloud_entity.node_entities[parent_index];

                        parent.children[node
                            .child_index
                            .index()
                            .expect("Trying to convert child index which isn't a valid index.")
                            as usize] = current_index;
                        parent.children_mask |= ChildrenMask::from(node.child_index);
                    }
                }

                // if there is a max depth, no need to go further
                if budget.max_depth.eq(&Some(node.depth)) {
                    // max depth is reached, no need to continue
                    continue;
                }

                #[cfg(feature = "trace")]
                let span_iter_children =
                    info_span!("compute_visible_nodes_stack", name = "iter_children").entered();

                // we have to process child nodes, sending flag `completely_visible` to prevent
                // useless visibility checks
                for i in node.children_mask.iter_one_bits() {
                    let child_id = &node.children[i as usize];
                    let Some(child) = octree.get_node(*child_id) else {
                        warn!("missing node in hierarchy, shouldn't happen");
                        continue;
                    };

                    // we skip empty loaded nodes
                    if matches!(child.status, PointCloudNodeStatus::Loaded)
                        && child.point_count == 0
                    {
                        continue;
                    }

                    let Some(aabb) = &child.aabb else {
                        continue;
                    };

                    let child_screen_pixel_radius =
                        compute_screen_pixel_radius(aabb, transform, camera_view);
                    let weight = child_screen_pixel_radius.unwrap_or(f32::MAX);

                    #[cfg(feature = "trace")]
                    let span_append_stack =
                        info_span!("compute_visible_nodes_stack", name = "append_stack").entered();

                    stack.push(StackedPointCloudNodeEntity {
                        entity,
                        asset_id,
                        octree,
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
            }
        }
    }
}

/// This system adds each visible [`PointCloudChunk3d`] entity ID to the [`VisibleEntities`]
/// component for each view. It also updates the [`ViewVisibility`] component on each of these
/// entities. This allows the chunks' render phases to be queued later in the render world, exactly
/// like standard meshes.
pub fn set_visible_point_cloud_chunk_visibility(
    mut views: Query<
        (
            &VisiblePointCloudFlatEntities,
            &VisiblePointCloudOctreeEntities,
            &mut VisibleEntities,
        ),
        With<Camera>,
    >,
    mut entities: Query<&mut ViewVisibility>,
) {
    for (
        visible_point_cloud_flat_entities,
        visible_point_cloud_octree_entities,
        mut visible_entities,
    ) in &mut views
    {
        // Retrieve or initialize the specific sub-list for PointCloudChunk3d inside Bevy's
        // VisibleEntities
        let visible_chunk_list = visible_entities.get_mut(TypeId::of::<PointCloudChunk3d>());

        // Iterate through each visible flat point cloud instance for this view
        for (&chunk_entity, _) in &visible_point_cloud_flat_entities.entities {
            // Append the chunk entity to Bevy's native visibility list for extraction
            visible_chunk_list.push(chunk_entity);

            // Update Bevy's internal ViewVisibility component to mark this chunk as visible
            // this frame
            if let Ok(mut view_visibility) = entities.get_mut(chunk_entity) {
                view_visibility.set_visible();
            }
        }

        // Iterate through each visible octree point cloud instance for this view
        for (_, point_cloud_entity) in &visible_point_cloud_octree_entities.entities {
            // Iterate through the visible octree nodes of this point cloud instance
            for node_entity in &point_cloud_entity.node_entities {
                // Keep only nodes that have a valid chunk entity assigned (ready to be rendered)
                if let Some(chunk_entity) = node_entity.entity {
                    // Append the chunk entity to Bevy's native visibility list for extraction
                    visible_chunk_list.push(chunk_entity);

                    // Update Bevy's internal ViewVisibility component to mark this chunk as visible
                    // this frame
                    if let Ok(mut view_visibility) = entities.get_mut(chunk_entity) {
                        view_visibility.set_visible();
                    }
                }
            }
        }
    }
}

/// Cleanup point cloud visibility after despawned
pub fn update_removed_point_clouds_visibility(
    mut removed_items: RemovedComponents<PointCloud3d>,
    mut visible_point_cloud_octrees_entities: Query<&mut VisiblePointCloudOctreeEntities>,
    mut cascades_visible_point_clouds_entities: Query<&mut CascadesVisiblePointCloudEntities>,
) {
    for entity in removed_items.read() {
        for mut visible_point_cloud_octree_entities in &mut visible_point_cloud_octrees_entities {
            visible_point_cloud_octree_entities.remove(&entity);
        }
        for mut cascades_visible_point_cloud_entities in &mut cascades_visible_point_clouds_entities
        {
            for (_, cascade_visible_point_cloud_entities) in
                &mut cascades_visible_point_cloud_entities.entities
            {
                for (_, visible_point_cloud_entities) in
                    cascade_visible_point_cloud_entities.iter_mut()
                {
                    visible_point_cloud_entities.remove(&entity);
                }
            }
        }
    }
}
