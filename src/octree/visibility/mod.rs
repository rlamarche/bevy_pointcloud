pub mod budget;
pub mod components;
pub mod filter;
mod heap_guard;
pub mod resources;
pub mod stack;

use super::asset::Octree;
use super::hierarchy::HierarchyNodeStatus;
use super::node::{NodeData, NodeStatus};
use super::server::resources::{LoadRequestType, OctreeLoadTasks};
use crate::octree::visibility::heap_guard::HeapGuard;
use crate::octree::visibility::resources::GlobalVisibleOctreeNodes;
use bevy_app::{App, Plugin, PostUpdate};
use bevy_asset::{AssetId, Assets};
use bevy_camera::primitives::{Aabb, Frustum};
use bevy_camera::visibility::VisibilitySystems::CheckVisibility;
use bevy_camera::visibility::{Visibility, VisibilityClass, VisibleEntities, add_visibility_class};
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
use budget::OctreeNodesBudget;
use components::*;
use filter::*;
use stack::*;
use std::any::TypeId;
use std::collections::{BinaryHeap, VecDeque};
use std::marker::PhantomData;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CheckOctreeNodesVisibility;

pub struct OctreeVisiblityPlugin<T, C, F = ScreenPixelRadiusFilter, B = ()>(
    PhantomData<fn() -> (T, C, F, B)>,
);

impl<T, C, F, B> OctreeVisiblityPlugin<T, C, F, B> {
    /// Visibility check diagnostic
    pub const VISIBILITY_CHECK_TIME: DiagnosticPath =
        DiagnosticPath::const_new("pcl_octree_visibility_check");

    /// Budget diagnostic
    pub const BUDGET: DiagnosticPath = DiagnosticPath::const_new("pcl_octree_budget");
}

impl<T, C, F, B> Default for OctreeVisiblityPlugin<T, C, F, B> {
    fn default() -> Self {
        OctreeVisiblityPlugin(PhantomData)
    }
}
impl<T, C, F, B> Plugin for OctreeVisiblityPlugin<T, C, F, B>
where
    T: NodeData,
    C: Component,
    for<'a> &'a C: Into<AssetId<Octree<T>>>,
    F: OctreeNodesFilter<T>,
    B: OctreeNodesBudget<T>,
{
    fn build(&self, app: &mut App) {
        app.register_diagnostic(
            Diagnostic::new(Self::VISIBILITY_CHECK_TIME)
                .with_suffix("ms")
                .with_max_history_length(DEFAULT_MAX_HISTORY_LENGTH)
                .with_smoothing_factor(2.0 / (DEFAULT_MAX_HISTORY_LENGTH as f64 + 1.0)),
        );
        app.register_diagnostic(
            Diagnostic::new(Self::BUDGET)
                .with_suffix("ms")
                .with_max_history_length(DEFAULT_MAX_HISTORY_LENGTH),
        );

        app.register_required_components::<C, Visibility>()
            .register_required_components::<C, VisibilityClass>()
            .register_required_components::<Camera, ViewVisibleOctreeNodes<T, C>>()
            .init_resource::<OctreeLoadTasks<T>>()
            .init_resource::<GlobalVisibleOctreeNodes<T, C>>()
            .add_systems(
                PostUpdate,
                check_octree_nodes_visibility::<T, C, F, B>.in_set(CheckOctreeNodesVisibility),
            )
            .configure_sets(
                PostUpdate,
                (CheckVisibility, CheckOctreeNodesVisibility).chain(),
            );

        app.world_mut()
            .register_component_hooks::<C>()
            .on_add(add_visibility_class::<C>);
    }
}

pub fn check_octree_nodes_visibility<T, C, F, B>(
    mut diagnostics: Diagnostics,
    _time: Res<Time<Real>>,
    entities: Query<(&C, &GlobalTransform)>,
    // TODO add a way to disable checking of a camera
    mut views: Query<
        (
            &VisibleEntities,
            &Camera,
            &Frustum,
            &GlobalTransform,
            &Projection,
            Option<&OctreeVisibilitySettings<T, F, B>>,
            &mut ViewVisibleOctreeNodes<T, C>,
        ),
        Without<SkipOctreeVisibility>,
    >,
    octrees: Res<Assets<Octree<T>>>,
    mut octree_load_tasks: ResMut<OctreeLoadTasks<T>>,
    mut priority_stack: Local<BinaryHeap<StackedOctreeNode<T>>>,
    mut global_visible_octree_nodes: ResMut<GlobalVisibleOctreeNodes<T, C>>,
) where
    T: NodeData,
    C: Component,
    for<'a> &'a C: Into<AssetId<Octree<T>>>,
    F: OctreeNodesFilter<T> + 'static,
    B: OctreeNodesBudget<T> + 'static,
{
    #[cfg(feature = "trace")]
    let _span = info_span!(
        "check_octree_nodes_visibility",
        name = "check_octree_nodes_visibility"
    )
    .entered();
    let start = Instant::now();
    octree_load_tasks.hierarchy_heap.clear();
    octree_load_tasks.node_heap.clear();

    // Clear previous iteration visible octree nodes
    global_visible_octree_nodes.clear();

    // for each view
    for (
        visible_entities,
        camera,
        frustum,
        camera_global_transform,
        camera_projection,
        visibility_settings,
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

        // create a local scoped priority stack reusing previous allocations
        let mut priority_stack = HeapGuard::new(&mut *priority_stack);

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
                octree: asset,
                node: node_root,
                weight: screen_pixel_radius.unwrap_or(f32::MAX).into(),
                screen_pixel_radius,
                // TODO check for this ?
                completely_visible: false,
                parent_index: None,
            });
        }

        let filter = visibility_settings.and_then(|settings| {
            settings
                .filter
                .as_ref()
                .and_then(|settings| <Option<F> as OctreeNodesFilter<T>>::new(settings))
        });

        let mut budget = visibility_settings.and_then(|settings| {
            settings
                .budget
                .as_ref()
                .and_then(|settings| <Option<B> as OctreeNodesBudget<T>>::new(settings))
        });

        compute_visible_nodes_stack(
            &camera_view,
            &filter,
            &mut budget,
            &mut priority_stack,
            &mut visible_octree_nodes,
            &mut global_visible_octree_nodes,
            &entities_transform,
            &mut octree_load_tasks,
        );

        diagnostics.add_measurement(&OctreeVisiblityPlugin::<T, C, B>::BUDGET, || budget.value());
    }

    let duration = start.elapsed();
    let msecs = duration.as_secs_f64() * 1000.0;

    diagnostics.add_measurement(
        &OctreeVisiblityPlugin::<T, C, B>::VISIBILITY_CHECK_TIME,
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
    #[cfg(feature = "trace")]
    let _span = info_span!("compute_screen_pixel_radius").entered();
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

            if distance < radius {
                return Some(f32::MAX);
            }

            Some(radius * proj_factor)
        }
        Projection::Orthographic(orthographic_projection) => {
            Some(radius * orthographic_projection.scale)
        }
        Projection::Custom(_) => None,
    }
}

fn compute_visible_nodes_stack<'a, T, C, F, B>(
    camera_view: &CameraView,
    filter: &F,
    budget: &mut Option<B>,
    stack: &mut BinaryHeap<StackedOctreeNode<T>>,
    view_visible_octree_nodes: &mut ViewVisibleOctreeNodes<T, C>,
    global_visible_octree_nodes: &mut GlobalVisibleOctreeNodes<T, C>,
    entities_transform: &HashMap<Entity, &GlobalTransform>,
    load_tasks: &mut OctreeLoadTasks<T>,
) where
    T: NodeData,
    C: Component,
    F: OctreeNodesFilter<T>,
    B: OctreeNodesBudget<T>,
{
    #[cfg(feature = "trace")]
    let _span = info_span!("compute_visible_nodes_stack", name = "main").entered();
    loop {
        let Some(StackedOctreeNode {
            entity,
            asset_id,
            octree,
            node,
            screen_pixel_radius,
            weight,
            mut completely_visible,
            parent_index,
        }) = stack.pop()
        else {
            break;
        };

        let (_, visible_nodes) = view_visible_octree_nodes.get_mut(entity);
        let Some(transform) = entities_transform.get(&entity) else {
            warn!("Missing transform for entity {}, skip", entity);
            continue;
        };

        let world_from_local = transform.affine();

        // get the current node future index
        let current_index = visible_nodes.len();

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
                #[cfg(feature = "trace")]
                let _span =
                    info_span!("compute_visible_nodes_stack", name = "hierarchy_only").entered();
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
                #[cfg(feature = "trace")]
                let _span = info_span!("compute_visible_nodes_stack", name = "loaded").entered();
                if budget.add_node(node) {
                    #[cfg(feature = "trace")]
                    let span_iter_children =
                        info_span!("compute_visible_nodes_stack", name = "iter_children").entered();
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

                        #[cfg(feature = "trace")]
                        let span_append_stack =
                            info_span!("compute_visible_nodes_stack", name = "append_stack")
                                .entered();
                        stack.push(StackedOctreeNode {
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

                    // add the current node because it is visible or partially visible
                    let child_index = node.hierarchy.child_index;
                    visible_nodes.push(node.into());
                    global_visible_octree_nodes.add_visible_octree_node(asset_id, node);

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
}

pub fn iter_one_bits(mask: u8) -> impl Iterator<Item = u8> {
    (0_u8..8).filter(move |&i| (mask & (1 << i)) != 0)
}
