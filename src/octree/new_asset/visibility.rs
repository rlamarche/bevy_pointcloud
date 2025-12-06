use crate::octree::new_asset::asset::NewOctree;
use crate::octree::new_asset::hierarchy::{
    HierarchyNodeData, HierarchyNodeStatus, HierarchyOctreeNode,
};
use crate::octree::new_asset::loader::OctreeLoader;
use crate::octree::new_asset::server::OctreeServerHelper;
use crate::octree::storage::NodeId;
use bevy_app::{App, Plugin, PreUpdate};
use bevy_asset::AssetId;
use bevy_camera::primitives::{Aabb, Frustum};
use bevy_camera::visibility::{Visibility, VisibilityClass, VisibleEntities, add_visibility_class, check_visibility};
use bevy_camera::{Camera, Projection};
use bevy_ecs::prelude::*;
use bevy_log::prelude::*;
use bevy_math::prelude::*;
use bevy_platform::collections::{HashMap, HashSet};
use bevy_reflect::TypePath;
use bevy_transform::prelude::*;
use potree::octree::node::iter_one_bits;
use std::any::TypeId;
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
        app.register_required_components::<C, Visibility>()
            .register_required_components::<C, VisibilityClass>()
            .register_required_components::<Camera, OctreesVisibility<H, T, C>>()
            .add_systems(PreUpdate, check_octree_nodes_visibility::<L, H, T, C>.after(check_visibility));

        app.world_mut()
            .register_component_hooks::<C>()
            .on_add(add_visibility_class::<C>);
    }
}

/// This component stores the visible nodes for each octree at view level (camera) in "main world".
#[derive(Debug, Component)]
pub struct OctreesVisibility<H, T, C>
where
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
    C: Component,
{
    pub octrees: HashMap<Entity, (AssetId<NewOctree<H, T>>, Vec<VisibleOctreeNode>)>,
    _phantom_data: PhantomData<fn() -> (H, T, C)>,
}

impl<H, T, C> Default for OctreesVisibility<H, T, C>
where
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
    C: Component,
{
    fn default() -> Self {
        Self {
            octrees: HashMap::default(),
            _phantom_data: PhantomData,
        }
    }
}

impl<H, T, C> OctreesVisibility<H, T, C>
where
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
    C: Component,
{
    pub fn get_mut(
        &mut self,
        entity: Entity,
    ) -> &mut (AssetId<NewOctree<H, T>>, Vec<VisibleOctreeNode>) {
        self.octrees.entry(entity).or_default()
    }

    pub fn clear_all(&mut self) {
        // Don't just nuke the hash table; we want to reuse allocations.
        for (asset_id, nodes) in self.octrees.values_mut() {
            *asset_id = Default::default();
            nodes.clear();
        }
    }
}

// #[derive(Clone, Debug)]
// pub struct OctreeVisibility<H, T>
// where
//     H: HierarchyNodeData,
//     T: Send + Sync + TypePath,
// {
//     pub asset_id: AssetId<NewOctree<H, T>>,
//     pub visible_nodes: Vec<VisibleOctreeNode>,
//     pub visible_hierarchy_nodes_to_load: Vec<NodeId>,
//     pub visible_nodes_to_load: Vec<NodeId>,
// }

/// Contains useful informations about a visible node
#[derive(Clone, Debug)]
pub struct VisibleOctreeNode {
    pub id: NodeId,
    pub children: [usize; 8],
    pub children_mask: u8,
    pub first_child_index: usize,
}

// pub struct VisibleHierarchyNode<H>
// where
//     H: HierarchyNodeData,
// {
//     pub index: usize,
//     pub children: [usize; 8],
//     pub children_mask: u8,
//     pub node: HierarchyOctreeNode<H>,
// }


impl<H> From<&HierarchyOctreeNode<H>> for VisibleOctreeNode
where
    H: HierarchyNodeData,
{
    fn from(value: &HierarchyOctreeNode<H>) -> Self {
        VisibleOctreeNode {
            id: value.id,
            children: [0_usize; 8],
            children_mask: 0b00000000,
            first_child_index: 0,
        }
    }
}

pub struct OctreeVisibilityInfo {
    pub visible_nodes: HashSet<NodeId>,
    pub lod_info: HashMap<NodeId, LodInfo>,
}

#[derive(Component)]
pub struct UnloadTracker {
    pub last_visible: HashMap<NodeId, u64>, // node_index -> last_frame_visible
}

pub struct LodInfo {
    pub distance_to_camera: f32,
    pub screen_size: f32,
    pub should_expand: bool,
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

pub fn check_octree_nodes_visibility<L, H, T, C>(
    entities: Query<(&C, &GlobalTransform)>,
    // TODO add a way to disable checking of a camera
    mut views: Query<(
        &VisibleEntities,
        &Camera,
        &Frustum,
        &GlobalTransform,
        &Projection,
        &mut OctreesVisibility<H, T, C>,
    )>,
    mut server: OctreeServerHelper<L, H, T>,
) where
    L: OctreeLoader<H>,
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
    C: Component,
    for<'a> &'a C: Into<AssetId<NewOctree<H, T>>>,
{
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

            // get the asset
            let Some(asset) = server.assets.get(component) else {
                warn!(
                    "Asset {} is missing, skip visibility check",
                    Into::<AssetId<NewOctree<H, T>>>::into(component)
                );
                continue;
            };

            let (asset_id, visible_nodes) = visible_octree_nodes.get_mut(*entity);

            // update the asset id
            *asset_id = component.into();

            let Some(hierarchy_root) = asset.hierarchy_root() else {
                warn!("Octree node has not yet hierarchy root loaded.");
                continue;
            };

            let filter = <ScreenPixelRadiusFilter as OctreeHierarchyFilter<H>>::new(150.0);
            let hierarchy_to_load = compute_visible_nodes(
                asset,
                hierarchy_root,
                global_transform,
                &camera_view,
                &filter,
                visible_nodes,
            );
            // info!(
            //     "computed {} visible nodes and {} nodes to load",
            //     visible_nodes.len(),
            //     hierarchy_to_load.len()
            // );

            for node_id in hierarchy_to_load {
                if let Err(error) = server.load_sub_hierarchy(component, node_id) {
                    warn!("Error loading sub hierarchy node: {}", error);
                }
            }
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

fn compute_visible_nodes<'a, H, T, F>(
    octree: &'a NewOctree<H, T>,
    node: &'a HierarchyOctreeNode<H>,
    transform: &GlobalTransform,
    camera_view: &CameraView,
    filter: &F,
    visible_nodes: &mut Vec<VisibleOctreeNode>,
) -> Vec<NodeId>
where
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
    F: OctreeHierarchyFilter<H>,
{
    let mut stack = VecDeque::from([(0_usize, node, false)]);
    // let mut visible_nodes = Vec::<VisibleHierarchyNode<H>>::new();
    let mut hierarchy_to_load = Vec::new();

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

                let child_index = node.child_index;
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

                    // if there is no first child, set it
                    // this is the first child index, because children are always processed in the same order
                    if parent.first_child_index == 0 {
                        parent.first_child_index = current_index;
                    }

                    parent.children[child_index] = current_index;
                    parent.children_mask |= 1 << node.child_index;

                }
            }
            HierarchyNodeStatus::Proxy => {
                hierarchy_to_load.push(node.id);
            }
            HierarchyNodeStatus::Loading => {
                // the node is already loading, nothing to do
            }
        }
    }

    hierarchy_to_load
}
