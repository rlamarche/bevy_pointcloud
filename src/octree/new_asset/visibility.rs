use crate::octree::new_asset::asset::NewOctree;
use crate::octree::new_asset::hierarchy::{
    HierarchyNodeData, HierarchyNodeStatus, HierarchyOctreeNode,
};
use crate::octree::new_asset::loader::OctreeLoader;
use crate::octree::storage::NodeId;
use bevy_app::{App, Plugin, PreUpdate};
use bevy_asset::{AssetId, Assets};
use bevy_camera::primitives::{Aabb, Frustum};
use bevy_camera::{Camera, Projection};
use bevy_ecs::prelude::*;
use bevy_log::prelude::*;
use bevy_math::prelude::*;
use bevy_reflect::TypePath;
use bevy_transform::prelude::*;
use potree::octree::node::iter_one_bits;
use std::collections::VecDeque;
use std::marker::PhantomData;
use thiserror::Error;

pub struct NewOctreeVisiblityPlugin<L, H, T, C, A>(PhantomData<fn() -> (L, H, T, C, A)>);

impl<L, H, T, C, A> Default for NewOctreeVisiblityPlugin<L, H, T, C, A> {
    fn default() -> Self {
        NewOctreeVisiblityPlugin(PhantomData)
    }
}
impl<L, H, T, C, A> Plugin for NewOctreeVisiblityPlugin<L, H, T, C, A>
where
    L: OctreeLoader<H> + 'static,
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
    C: Component,
    for<'a> &'a C: Into<AssetId<NewOctree<H, T>>>,
    A: Send + Sync + 'static,
{
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, check_octree_nodes_visibility::<H, T, C>);
    }
}

pub trait OctreeHierarchyFilter<H>: Send + Sync
where
    H: HierarchyNodeData,
{
    type Settings: Send + Sync;

    fn new(settings: Self::Settings) -> Self;

    fn filter(
        &self,
        node: &HierarchyOctreeNode<H>,
        global_transform: &GlobalTransform,
        camera_view: &CameraView,
    ) -> bool;
}

pub struct ScreenPixelRadiusFilter {
    min_radius: f32,
}

impl<H> OctreeHierarchyFilter<H> for ScreenPixelRadiusFilter
where
    H: HierarchyNodeData,
{
    type Settings = f32;

    fn new(min_radius: Self::Settings) -> Self {
        Self { min_radius }
    }

    fn filter(
        &self,
        node: &HierarchyOctreeNode<H>,
        global_transform: &GlobalTransform,
        camera_view: &CameraView,
    ) -> bool {
        let radius = compute_screen_pixel_radius(&node.bounding_box, global_transform, camera_view);

        if let Some(radius) = radius
            && radius < self.min_radius
        {
            false
        } else {
            true
        }
    }
}

#[derive(Clone, Debug, Error)]
pub enum BudgetError {
    #[error("Budget for octree hierarchy has been reached.")]
    NoBudgetLeft,
}

pub trait OctreeHierarchyBudget<H>: Send + Sync
where
    H: HierarchyNodeData,
{
    type Settings: Send + Sync;

    fn new(settings: Self::Settings) -> Self;

    fn check(&self, node: &HierarchyOctreeNode<H>) -> bool;

    fn add_node(&mut self, node: &HierarchyOctreeNode<H>) -> Result<(), BudgetError>;
}

pub fn check_octree_nodes_visibility<H, T, C>(
    assets: Res<Assets<NewOctree<H, T>>>,
    entities: Query<(&C, &GlobalTransform)>,
    // TODO add a way to disable checking of a camera
    views: Query<(&Camera, &Frustum, &GlobalTransform, &Projection)>,
) where
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
    C: Component,
    for<'a> &'a C: Into<AssetId<NewOctree<H, T>>>,
{
    // for each view
    for (camera, frustum, camera_global_transform, camera_projection) in views.iter() {
        let camera_view = CameraView {
            global_transform: camera_global_transform,
            frustum,
            projection: camera_projection,
            physical_target_size: camera.physical_target_size(),
        };

        // for each point cloud instance
        for (component, global_transform) in entities {
            // get the asset
            let Some(asset) = assets.get(component) else {
                warn!(
                    "Asset {} is missing, skip visibility check",
                    Into::<AssetId<NewOctree<H, T>>>::into(component)
                );
                continue;
            };

            let Some(hierarchy_root) = asset.hierarchy_root() else {
                warn!("Octree node has not yet hierarchy root loaded.");
                continue;
            };

            let filter = <ScreenPixelRadiusFilter as OctreeHierarchyFilter<H>>::new(150.0);
            let (visible_nodes, nodes_to_load) = compute_visible_nodes(
                asset,
                hierarchy_root,
                global_transform,
                &camera_view,
                &filter,
            );
            info!(
                "computed {} visible nodes and {} nodes to load",
                visible_nodes.len(),
                nodes_to_load.len()
            );
        }
    }
}

#[derive(Clone, Debug)]
pub struct CameraView<'a> {
    pub global_transform: &'a GlobalTransform,
    pub frustum: &'a Frustum,
    pub projection: &'a Projection,
    pub physical_target_size: Option<UVec2>,
}

fn compute_screen_pixel_radius<'a>(
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

fn compute_visible_nodes<'a, H, T, F>(
    octree: &'a NewOctree<H, T>,
    node: &'a HierarchyOctreeNode<H>,
    transform: &GlobalTransform,
    camera_view: &CameraView,
    filter: &F,
) -> (Vec<VisibleHierarchyNode<H>>, Vec<NodeId>)
where
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
    F: OctreeHierarchyFilter<H>,
{
    let mut stack = VecDeque::from([(0_usize, node, false)]);
    let mut visible_nodes = Vec::<VisibleHierarchyNode<H>>::new();
    let mut nodes_to_load = Vec::new();

    let world_from_local = transform.affine();

    while let Some((parent_index, node, mut completely_visible)) = stack.pop_front() {
        // if node.node_type.has_points() && node.num_points == 0 {
        //     // skip nodes with no points
        //     continue;
        // }

        // get the current node future index
        let current_index = visible_nodes.len();

        // check node visibility
        if !filter.filter(node, transform, camera_view) {
            continue;
        }

        if !completely_visible {
            // check if the node aabb against the frustum
            let model_sphere = bevy_camera::primitives::Sphere {
                center: world_from_local.transform_point3a(node.bounding_box.center),
                radius: transform.radius_vec3a(node.bounding_box.half_extents),
            };

            // Do quick sphere-based frustum culling
            if !camera_view.frustum.intersects_sphere(&model_sphere, false) {
                // this node is not visible, continue
                continue;
            }

            // Check if the aabb is completly inside the frustum
            if camera_view
                .frustum
                .contains_aabb(&node.bounding_box, &world_from_local)
            {
                // mark as completely visible to prevent later checks
                completely_visible = true;

                // else, do oriented bounding box frustum culling
            } else if !camera_view.frustum.intersects_obb(
                &node.bounding_box,
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
            HierarchyNodeStatus::Loaded => {
                // we have to process child nodes, sending flag `completely_visible` to prevent useless visibility checks
                for i in iter_one_bits(node.children_mask) {
                    let child = &node.children[i];
                    let Some(child) = octree.hierarchy_node(*child) else {
                        warn!("missing node in hierarchy, shouldn't happen");
                        continue;
                    };
                    stack.push_back((current_index, child, completely_visible));
                }

                // add the current node because it is visible or partially visible

                let visible_node = VisibleHierarchyNode {
                    node: (*node).clone(),
                    index: current_index,
                    children: [0; 8],
                    children_mask: 0,
                };

                let child_index = node.child_index;
                visible_nodes.push(visible_node);

                // if there is a parent, add it to the children array on an empty space
                if parent_index < current_index {
                    let parent = &mut visible_nodes[parent_index];
                    parent.children[child_index] = current_index;
                    parent.children_mask |= 1 << node.child_index;
                }
            }
            HierarchyNodeStatus::Proxy => {
                nodes_to_load.push(node.id);
            }
        }
    }

    (visible_nodes, nodes_to_load)
}

pub struct VisibleHierarchyNode<H>
where
    H: HierarchyNodeData,
{
    pub index: usize,
    pub children: [usize; 8],
    pub children_mask: u8,
    pub node: HierarchyOctreeNode<H>,
}
