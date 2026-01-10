pub mod budget;
pub mod components;
pub mod filter;
pub mod stack;

use super::asset::Octree;
use super::hierarchy::HierarchyNodeStatus;
use super::node::{NodeData, NodeStatus, OctreeNode};
use super::server::resources::{LoadRequestType, OctreeLoadTasks};
use crate::octree::storage::NodeId;
use bevy_app::{App, Plugin, PreUpdate};
use bevy_asset::{AssetId, Assets};
use bevy_camera::primitives::{Aabb, Frustum};
use bevy_camera::visibility::{
    Visibility, VisibilityClass, VisibleEntities, add_visibility_class, check_visibility,
};
use bevy_camera::{Camera, Projection};
use bevy_diagnostic::{
    DEFAULT_MAX_HISTORY_LENGTH, Diagnostic, DiagnosticPath, Diagnostics, RegisterDiagnostic,
};
use bevy_ecs::prelude::*;
use bevy_log::prelude::*;
use bevy_math::prelude::*;
use bevy_platform::collections::HashMap;
use bevy_platform::time::Instant;
use bevy_time::{Real, Time};
use bevy_transform::prelude::*;
use components::*;
use filter::*;
use stack::*;
use std::any::TypeId;
use std::collections::{BinaryHeap, VecDeque};
use std::marker::PhantomData;

pub struct OctreeVisiblityPlugin<T, C, A>(PhantomData<fn() -> (T, C, A)>);

impl<T, C, A> OctreeVisiblityPlugin<T, C, A> {
    /// Visibility check diagnostic
    pub const VISIBILITY_CHECK_TIME: DiagnosticPath =
        DiagnosticPath::const_new("pcl_octree_visibility_check");
}

impl<T, C, A> Default for OctreeVisiblityPlugin<T, C, A> {
    fn default() -> Self {
        OctreeVisiblityPlugin(PhantomData)
    }
}
impl<T, C, A> Plugin for OctreeVisiblityPlugin<T, C, A>
where
    T: NodeData,
    C: Component,
    for<'a> &'a C: Into<AssetId<Octree<T>>>,
    A: Send + Sync + 'static,
{
    fn build(&self, app: &mut App) {
        app.register_diagnostic(
            Diagnostic::new(Self::VISIBILITY_CHECK_TIME)
                .with_suffix("ms")
                .with_max_history_length(DEFAULT_MAX_HISTORY_LENGTH)
                .with_smoothing_factor(2.0 / (DEFAULT_MAX_HISTORY_LENGTH as f64 + 1.0)),
        );

        app.register_required_components::<C, Visibility>()
            .register_required_components::<C, VisibilityClass>()
            .register_required_components::<Camera, OctreesVisibility<T, C>>()
            .init_resource::<OctreeLoadTasks<T>>()
            .add_systems(
                PreUpdate,
                check_octree_nodes_visibility::<T, C, A>.after(check_visibility),
            );

        app.world_mut()
            .register_component_hooks::<C>()
            .on_add(add_visibility_class::<C>);
    }
}

pub fn check_octree_nodes_visibility<T, C, A>(
    mut diagnostics: Diagnostics,
    time: Res<Time<Real>>,
    entities: Query<(&C, &GlobalTransform)>,
    // TODO add a way to disable checking of a camera
    mut views: Query<(
        &VisibleEntities,
        &Camera,
        &Frustum,
        &GlobalTransform,
        &Projection,
        &mut OctreesVisibility<T, C>,
    )>,
    octrees: Res<Assets<Octree<T>>>,
    mut octree_load_tasks: ResMut<OctreeLoadTasks<T>>,
    mut priority_stack: Local<BinaryHeap<StackedOctreeNode<T>>>,
) where
    T: NodeData,
    C: Component,
    for<'a> &'a C: Into<AssetId<Octree<T>>>,
    A: Send + Sync + 'static,
{
    let start = Instant::now();

    // for each view
    for (
        visible_entities,
        camera,
        frustum,
        camera_global_transform,
        camera_projection,
        mut visible_octree_nodes,
    ) in &mut views
    {
        if !camera.is_active {
            continue;
        }

        // Reset previously computed visibility
        visible_octree_nodes.clear_all();

        let camera_view = CameraView {
            global_transform: camera_global_transform,
            frustum,
            projection: camera_projection,
            physical_target_size: camera.physical_target_size(),
        };

        // get all visible octrees
        let visible_octree_entities = visible_entities.get(TypeId::of::<C>());

        // clear the priority stack
        priority_stack.clear();

        let mut entities_transform = HashMap::new();

        // for each visible octree
        for entity in visible_octree_entities {
            let (component, global_transform) = match entities.get(*entity) {
                Ok(item) => item,
                Err(error) => {
                    warn!(
                        "Unable to read octree entity for computing nodes visibility: {:#}",
                        error
                    );
                    continue;
                }
            };

            entities_transform.insert(*entity, global_transform);

            // get the asset
            let Some(asset) = octrees.get(component) else {
                // warn!(
                //     "Asset {} is missing, skip visibility check",
                //     Into::<AssetId<Octree<T>>>::into(component)
                // );
                continue;
            };

            let (asset_id, _) = visible_octree_nodes.get_mut(*entity);

            // update the asset id
            *asset_id = component.into();

            let Some(root_id) = asset.root_id() else {
                warn!("Octree node has not yet hierarchy root loaded.");
                continue;
            };

            let Some(node_root) = asset.node_root() else {
                warn!("Octree node has not yet hierarchy root loaded.");
                continue;
            };

            let screen_pixel_radius = compute_screen_pixel_radius(
                &node_root.hierarchy.bounding_box,
                global_transform,
                &camera_view,
            );

            // add the root octree to the priority stack
            priority_stack.push(StackedOctreeNode {
                entity: *entity,
                asset_id: *asset_id,
                node_id: root_id,
                weight: screen_pixel_radius.unwrap_or(f32::MAX).into(),
                screen_pixel_radius,
                // TODO check for this ?
                completely_visible: false,
                parent_index: None,
            });

            // let filter = <ScreenPixelRadiusFilter as OctreeHierarchyFilter<H>>::new(150.0);
            // let hierarchy_to_load = compute_visible_nodes(
            //     asset,
            //     hierarchy_root,
            //     global_transform,
            //     &camera_view,
            //     &filter,
            //     visible_nodes,
            // );
            // // info!(
            // //     "computed {} visible nodes and {} nodes to load",
            // //     visible_nodes.len(),
            // //     hierarchy_to_load.len()
            // // );
            //
            // for node_id in hierarchy_to_load {
            //     if let Err(error) = server.load_sub_hierarchy(component, node_id) {
            //         warn!("Error loading sub hierarchy node: {}", error);
            //     }
            // }
        }

        // TODO make filter configurable
        let filter = <ScreenPixelRadiusFilter as OctreeHierarchyFilter<T>>::new(150.0);
        compute_visible_nodes_stack(
            octrees.as_ref(),
            &camera_view,
            &filter,
            &mut priority_stack,
            &mut visible_octree_nodes,
            &entities_transform,
            &mut octree_load_tasks,
        );
    }

    let duration = start.elapsed();
    let msecs = duration.as_secs_f64() * 1000.0;

    diagnostics.add_measurement(
        &OctreeVisiblityPlugin::<T, C, A>::VISIBILITY_CHECK_TIME,
        || msecs,
    );
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
    let radius = (aabb.max() - aabb.min()).length() / 2.0;

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

            Some(radius * proj_factor)
        }
        Projection::Orthographic(orthographic_projection) => {
            Some(radius * orthographic_projection.scale)
        }
        Projection::Custom(_) => None,
    }
}

// fn compute_distance_and_screen_size(
//     aabb: &Aabb,
//     transform: &GlobalTransform,
//     camera_view: &CameraView,
// ) -> Option<(f32, f32)> {
//     let radius = (aabb.max() - aabb.min()).length() / 2.0;
//
//     let center = transform.affine().transform_point3a(aabb.center);
//     let camera_center = Into::<Vec3A>::into(camera_view.global_transform.translation());
//     let distance = (center - camera_center).length();
//
//     match &camera_view.projection {
//         Projection::Perspective(perspective_projection) => {
//             let Some(physical_target_size) = &camera_view.physical_target_size else {
//                 return None;
//             };
//
//             let slope = (perspective_projection.fov / 2.0).atan();
//             let proj_factor = (0.5 * physical_target_size.y as f32) / (slope * distance);
//
//             Some((distance, radius * proj_factor))
//         }
//         Projection::Orthographic(orthographic_projection) => {
//             Some((distance, radius * orthographic_projection.scale))
//         }
//         Projection::Custom(_) => None,
//     }
// }

fn compute_visible_nodes_stack<'a, T, C, F>(
    octrees: &Assets<Octree<T>>,
    camera_view: &CameraView,
    filter: &F,
    stack: &mut BinaryHeap<StackedOctreeNode<T>>,
    visible_octree_nodes: &mut OctreesVisibility<T, C>,
    entities_transform: &HashMap<Entity, &GlobalTransform>,
    load_tasks: &mut OctreeLoadTasks<T>,
) where
    T: NodeData,
    C: Component,
    F: OctreeHierarchyFilter<T>,
{
    loop {
        let Some(StackedOctreeNode {
            entity,
            asset_id,
            node_id,
            screen_pixel_radius,
            weight,
            mut completely_visible,
            parent_index,
        }) = stack.pop()
        else {
            break;
        };

        let (_, visible_nodes) = visible_octree_nodes.get_mut(entity);
        let Some(transform) = entities_transform.get(&entity) else {
            warn!("Missing transform for entity {}, skip", entity);
            continue;
        };
        let Some(octree) = octrees.get(asset_id) else {
            warn!("Missing asset {}", asset_id);
            continue;
        };
        let Some(node) = octree.node(node_id) else {
            warn!("Missing node {} in asset {}", node_id, asset_id);
            continue;
        };

        let world_from_local = transform.affine();

        // get the current node future index
        let current_index = visible_nodes.len();

        // check node visibility
        if !filter.filter(node, transform, camera_view, screen_pixel_radius) {
            continue;
        }

        if !completely_visible {
            // check if the node aabb against the frustum
            let model_sphere = bevy_camera::primitives::Sphere {
                center: world_from_local.transform_point3a(node.hierarchy.bounding_box.center),
                radius: transform.radius_vec3a(node.hierarchy.bounding_box.half_extents),
            };

            // Do quick sphere-based frustum culling
            if !camera_view.frustum.intersects_sphere(&model_sphere, false) {
                // this node is not visible, continue
                continue;
            }

            // Check if the aabb is completly inside the frustum
            if camera_view
                .frustum
                .contains_aabb(&node.hierarchy.bounding_box, &world_from_local)
            {
                // mark as completely visible to prevent later checks
                completely_visible = true;

                // else, do oriented bounding box frustum culling
            } else if !camera_view.frustum.intersects_obb(
                &node.hierarchy.bounding_box,
                &world_from_local,
                true,
                // we do not set a distance limit because too small might have been already filtered
                false,
            ) {
                // the node is completely outside the frustum, ignore it
                continue;
            }
        }

        match node.status {
            NodeStatus::HierarchyOnly => {
                match node.hierarchy.status {
                    HierarchyNodeStatus::Proxy => {
                        load_tasks.queue_load_request(
                            asset_id,
                            node.hierarchy.id,
                            weight,
                            LoadRequestType::Hierarchy,
                        );
                    }
                    HierarchyNodeStatus::Loading => {
                        // the node hierarchy is already loading, nothing to do
                    }
                    HierarchyNodeStatus::Loaded => {
                        load_tasks.queue_load_request(
                            asset_id,
                            node.hierarchy.id,
                            weight,
                            LoadRequestType::NodeData,
                        );
                    }
                }
            }
            NodeStatus::Loading => {
                // the node data is already loading, nothing to do
            }
            NodeStatus::Loaded => {
                // we have to process child nodes, sending flag `completely_visible` to prevent useless visibility checks
                for i in iter_one_bits(node.hierarchy.children_mask) {
                    let child_id = &node.hierarchy.children[i as usize];
                    let Some(child) = octree.node(*child_id) else {
                        warn!("missing node in hierarchy, shouldn't happen");
                        continue;
                    };

                    let child_screen_pixel_radius = compute_screen_pixel_radius(
                        &child.hierarchy.bounding_box,
                        transform,
                        camera_view,
                    );
                    let weight = child_screen_pixel_radius.unwrap_or(f32::MAX);

                    stack.push(StackedOctreeNode {
                        entity,
                        asset_id,
                        node_id: *child_id,
                        screen_pixel_radius: child_screen_pixel_radius,
                        weight: weight.into(),
                        completely_visible,
                        parent_index: Some(current_index),
                    });
                }

                // add the current node because it is visible or partially visible
                let child_index = node.hierarchy.child_index;
                visible_nodes.push(node.into());

                // if there is a parent, add it to the visible children array
                if let Some(parent_index) = parent_index {
                    let parent = &mut visible_nodes[parent_index];

                    parent.children[child_index as usize] = current_index;
                    parent.children_mask |= 1 << node.hierarchy.child_index;
                }
            }
        }
    }
}

fn compute_visible_nodes<'a, T, F>(
    octree: &'a Octree<T>,
    node: &'a OctreeNode<T>,
    transform: &GlobalTransform,
    camera_view: &CameraView,
    filter: &F,
    visible_nodes: &mut Vec<VisibleOctreeNode>,
) -> (Vec<NodeId>, Vec<NodeId>)
where
    T: NodeData,
    F: OctreeHierarchyFilter<T>,
{
    let mut stack = VecDeque::from([(0_usize, node, false)]);
    // let mut visible_nodes = Vec::<VisibleHierarchyNode<H>>::new();
    let mut hierarchy_to_load = Vec::new();
    let mut node_data_to_load = Vec::new();

    let world_from_local = transform.affine();

    while let Some((parent_index, node, mut completely_visible)) = stack.pop_front() {
        // if node.node_type.has_points() && node.num_points == 0 {
        //     // skip nodes with no points
        //     continue;
        // }

        // get the current node future index
        let current_index = visible_nodes.len();

        let screen_pixel_radius =
            compute_screen_pixel_radius(&node.hierarchy.bounding_box, transform, camera_view);

        // check node visibility
        if !filter.filter(node, transform, camera_view, screen_pixel_radius) {
            continue;
        }

        if !completely_visible {
            // check if the node aabb against the frustum
            let model_sphere = bevy_camera::primitives::Sphere {
                center: world_from_local.transform_point3a(node.hierarchy.bounding_box.center),
                radius: transform.radius_vec3a(node.hierarchy.bounding_box.half_extents),
            };

            // Do quick sphere-based frustum culling
            if !camera_view.frustum.intersects_sphere(&model_sphere, false) {
                // this node is not visible, continue
                continue;
            }

            // Check if the aabb is completly inside the frustum
            if camera_view
                .frustum
                .contains_aabb(&node.hierarchy.bounding_box, &world_from_local)
            {
                // mark as completely visible to prevent later checks
                completely_visible = true;

                // else, do oriented bounding box frustum culling
            } else if !camera_view.frustum.intersects_obb(
                &node.hierarchy.bounding_box,
                &world_from_local,
                true,
                // we do not set a distance limit because too small might have been already filtered
                false,
            ) {
                // the node is completely outside the frustum, ignore it
                continue;
            }
        }

        match node.status {
            NodeStatus::HierarchyOnly => {
                match node.hierarchy.status {
                    HierarchyNodeStatus::Proxy => {
                        hierarchy_to_load.push(node.hierarchy.id);
                    }
                    HierarchyNodeStatus::Loading => {
                        // the node hierarchy is already loading, nothing to do
                    }
                    HierarchyNodeStatus::Loaded => {
                        // mark node data to load
                        node_data_to_load.push(node.hierarchy.id);
                    }
                }
            }
            NodeStatus::Loading => {
                // the node data is already loading, nothing to do
            }
            NodeStatus::Loaded => {
                // we have to process child nodes, sending flag `completely_visible` to prevent useless visibility checks
                for i in iter_one_bits(node.hierarchy.children_mask) {
                    let child = &node.hierarchy.children[i as usize];
                    let Some(child) = octree.node(*child) else {
                        warn!("missing node in octree, shouldn't happen");
                        continue;
                    };
                    stack.push_back((current_index, child, completely_visible));
                }

                // add the current node because it is visible or partially visible

                let child_index = node.hierarchy.child_index;
                visible_nodes.push(node.into());

                // let visible_node = VisibleHierarchyNode {
                //     node: (*node).clone(),
                //     index: current_index,
                //     children: [0; 8],
                //     children_mask: 0,
                // };

                // visible_nodes.push(visible_node);

                // if there is a parent, add it to the children array on an empty space
                if parent_index < current_index {
                    let parent = &mut visible_nodes[parent_index];

                    parent.children[child_index as usize] = current_index;
                    parent.children_mask |= 1 << node.hierarchy.child_index;
                }
            }
        }
    }

    (hierarchy_to_load, node_data_to_load)
}

pub fn iter_one_bits(mask: u8) -> impl Iterator<Item = u8> {
    (0_u8..8).filter(move |&i| (mask & (1 << i)) != 0)
}
