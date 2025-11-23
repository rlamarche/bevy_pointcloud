use crate::octree::asset::{NodeId, Octree, OctreeNode};
use crate::octree::visibility::extract::{
    ExtractOctreeNode, ExtractedOctreeNodes, extract_render_octree_nodes,
};
use crate::pointcloud_octree::component::PointCloudOctree3d;
use bevy_app::{App, Plugin, PostUpdate, SubApp};
use bevy_asset::{AssetId, Assets};
use bevy_camera::visibility::{
    Visibility, VisibilityClass, VisibleEntities, add_visibility_class, check_visibility,
};
use bevy_camera::{
    Camera, Projection,
    primitives::{Frustum, Sphere},
};
use bevy_ecs::prelude::*;
use bevy_ecs::query::QueryItem;
use bevy_ecs::schedule::ScheduleConfigs;
use bevy_ecs::system::ScheduleSystem;
use bevy_log::prelude::*;
use bevy_math::UVec2;
use bevy_math::prelude::*;
use bevy_platform::collections::HashMap;
use bevy_reflect::TypePath;
use bevy_render::camera::extract_cameras;
use bevy_render::extract_component::ExtractComponent;
use bevy_render::sync_world::RenderEntity;
use bevy_render::{Extract, ExtractSchedule, Render, RenderApp, RenderSystems};
use bevy_transform::components::GlobalTransform;
use core::any::TypeId;
use limiter::{
    RenderOctreeNodesBytesPerFrame, RenderOctreeNodesBytesPerFrameLimiter,
    extract_render_asset_bytes_per_frame, reset_render_asset_bytes_per_frame,
};
use prepare::{PrepareNextFrameOctreeNodes, RenderOctreeNode, RenderOctrees, prepare_assets};
use slab::Slab;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::marker::PhantomData;

pub mod extract;
mod limiter;
pub mod prepare;

/// This plugin extracts visible octree nodes from the "app world" into the "render world"
/// and prepares them for the GPU. They can be accessed from the [`RenderVisibleOctreeNodes`] resource.
///
/// The `T` generic parameter refers to the type of octree node we are talking about.
/// Because an asset is an octree, we need a component to reference it.
/// This is the role of `C` generic parameter, which is the component used to find the referring octree asset.
/// So, the `C` generic parameter has to implement `Into<AssetId<Octree<T>>`.
///
/// The `A` generic parameter represents the octree node viewed by the gpu.
/// It has to implement the [`RenderOctreeNode`] trait to determine how octree nodes are converted in gpu format.
///
/// The `AFTER` generic parameter can be used to specify that `A::prepare_octree_node` should not be run until
/// `prepare_assets::<AFTER>` has completed. This allows the `prepare_octree_node` function to depend on another
/// prepared [`RenderOctreeNode`].
pub struct ExtractVisibleOctreeNodesPlugin<T, C, A, AFTER = ()>(
    PhantomData<fn() -> (T, C, A, AFTER)>,
);

impl<T, C, A, AFTER> Default for ExtractVisibleOctreeNodesPlugin<T, C, A, AFTER> {
    fn default() -> Self {
        ExtractVisibleOctreeNodesPlugin(PhantomData)
    }
}

impl<T, C, A, AFTER> Plugin for ExtractVisibleOctreeNodesPlugin<T, C, A, AFTER>
where
    T: ExtractOctreeNode,
    C: Component,
    for<'a> &'a C: Into<AssetId<Octree<T>>>,
    A: RenderOctreeNode<ExtractedOctreeNode = T::Out, SourceOctreeNode = T>,
    AFTER: RenderOctreeDependency + 'static,
{
    fn build(&self, app: &mut App) {
        app.init_resource::<RenderOctreeNodesBytesPerFrame>();

        // makes [`Octree<T>`] an entity checked for visibility by [`bevy_camera`].
        app.register_required_components::<C, Visibility>()
            .register_required_components::<C, VisibilityClass>()
            .register_required_components::<Camera, VisibleOctreeNodes<T, C>>()
            .add_systems(
                PostUpdate,
                check_octree_node_visibility::<T, C>.after(check_visibility),
            );

        app.world_mut()
            .register_component_hooks::<C>()
            .on_add(add_visibility_class::<C>);

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.init_resource::<RenderOctreeNodesBytesPerFrameLimiter>();
        render_app
            .add_systems(ExtractSchedule, extract_render_asset_bytes_per_frame)
            .add_systems(
                Render,
                reset_render_asset_bytes_per_frame.in_set(RenderSystems::Cleanup),
            );

        render_app
            .init_resource::<ExtractedOctreeNodes<T>>()
            .init_resource::<RenderOctrees<A>>()
            .init_resource::<PrepareNextFrameOctreeNodes<A>>()
            .init_resource::<RenderOctreeIndex<C>>()
            .add_systems(
                ExtractSchedule,
                (
                    extract_visible_octree_nodes::<T, C>.after(extract_cameras),
                    extract_render_octree_nodes::<T, C, A>
                        .after(extract_visible_octree_nodes::<T, C>),
                ),
            );

        AFTER::register_system(
            render_app,
            prepare_assets::<A, C>.in_set(RenderSystems::PrepareAssets),
        );
    }
}

// helper to allow specifying dependencies between render assets
pub trait RenderOctreeDependency {
    fn register_system(render_app: &mut SubApp, system: ScheduleConfigs<ScheduleSystem>);
}

impl RenderOctreeDependency for () {
    fn register_system(render_app: &mut SubApp, system: ScheduleConfigs<ScheduleSystem>) {
        render_app.add_systems(Render, system);
    }
}

/// Contains useful informations about a visible node
#[derive(Clone, Debug)]
pub struct VisibleOctreeNode {
    pub id: NodeId,
    pub children: [NodeId; 8],
    pub children_mask: u8,
    pub first_child_index: usize,
}

impl<T> From<&OctreeNode<T>> for VisibleOctreeNode
where
    T: Send + Sync + TypePath,
{
    fn from(value: &OctreeNode<T>) -> Self {
        VisibleOctreeNode {
            id: value.id,
            children: value.children.clone(),
            children_mask: 0b00000000,
            first_child_index: 0,
        }
    }
}

/// This resource stores the octree mapping to index in render world.
#[derive(Debug, Resource)]
pub struct RenderOctreeIndex<C>
where
    C: Component,
{
    pub octrees_slab: Slab<Entity>,
    pub octrees_index: HashMap<Entity, usize>,
    _phantom_data: PhantomData<C>,
}

impl<C: Component> FromWorld for RenderOctreeIndex<C> {
    fn from_world(_: &mut World) -> Self {
        RenderOctreeIndex {
            octrees_slab: Slab::new(),
            octrees_index: HashMap::new(),
            _phantom_data: PhantomData,
        }
    }
}

impl<C: Component> RenderOctreeIndex<C> {
    /// Add octree entity to index, if it already exists, does nothing.
    pub fn add_octree(&mut self, entity: Entity) -> usize {
        // TODO cleanup old octrees from the slab (if removed ?)
        *self
            .octrees_index
            .entry(entity)
            .or_insert_with(|| self.octrees_slab.insert(entity))
    }

    /// Removes an entity from the index.
    pub fn remove_octree(&mut self, entity: Entity) -> Option<usize> {
        if let Some(index) = self.octrees_index.remove(&entity) {
            self.octrees_slab.remove(index);
            Some(index)
        } else {
            None
        }
    }

    pub fn get_octree_index(&self, entity: Entity) -> Option<usize> {
        self.octrees_index.get(&entity).copied()
    }
}

/// This component stores the visible nodes for each octree at view level (camera) in "main world".
#[derive(Debug, Component)]
pub struct VisibleOctreeNodes<T, C>
where
    T: Send + Sync + TypePath,
    C: Component,
{
    pub octrees: HashMap<Entity, (AssetId<Octree<T>>, Vec<VisibleOctreeNode>)>,
    _phantom_data: PhantomData<fn() -> (T, C)>,
}

impl<T, C> Default for VisibleOctreeNodes<T, C>
where
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

impl<T, C> VisibleOctreeNodes<T, C>
where
    T: Send + Sync + TypePath,
    C: Component,
{
    pub fn get_mut(&mut self, entity: Entity) -> &mut (AssetId<Octree<T>>, Vec<VisibleOctreeNode>) {
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

/// System computing visible nodes of each octree, each frame.
///
/// It checks only entities having a [`ViewVisibility`] component (from [`bevy_camera`].
pub fn check_octree_node_visibility<T, C>(
    mut view_query: Query<(
        &VisibleEntities,
        &Frustum,
        &Camera,
        &GlobalTransform,
        &Projection,
        &mut VisibleOctreeNodes<T, C>,
    )>,
    octree_entities: Query<(&C, &GlobalTransform)>,
    pointcloud_octrees: Res<Assets<Octree<T>>>,
) where
    C: Component,
    T: Send + Sync + TypePath,
    for<'a> &'a C: Into<AssetId<Octree<T>>>,
{
    for (
        visible_entities,
        frustum,
        camera,
        camera_transform,
        projection,
        mut visible_octree_nodes,
    ) in &mut view_query
    {
        if !camera.is_active {
            continue;
        }

        // clear all previous visible nodes
        visible_octree_nodes.clear_all();

        let physical_target_size = camera.physical_target_size();

        // get all visible octrees
        let visible_octree_entities = visible_entities.get(TypeId::of::<C>());

        // then check each of one's nodes
        for entity in visible_octree_entities {
            // load entity
            let (octree_component, octree_transform) = match octree_entities.get(*entity) {
                Ok(item) => item,
                Err(error) => {
                    warn!(
                        "Unable to read octree entity for computing nodes visibility: {:#}",
                        error
                    );
                    continue;
                }
            };

            let (asset_id, nodes) = visible_octree_nodes.get_mut(*entity);

            // update the asset id
            *asset_id = octree_component.into();

            // load octree asset
            let Some(octree) = pointcloud_octrees.get(*asset_id) else {
                warn!("Missing point cloud octree in assets");
                continue;
            };

            // compute visible nodes and store them
            compute_visible_nodes(
                octree,
                octree_transform,
                camera_transform,
                frustum,
                projection,
                &physical_target_size,
                nodes,
            );
        }
    }
}

/// Computes screen pixel radius based on camera projection
fn compute_screen_pixel_radius<T>(
    node: &OctreeNode<T>,
    octree_transform: &GlobalTransform,
    camera_transform: &GlobalTransform,
    projection: &Projection,
    physical_target_size: &Option<UVec2>,
) -> Option<f32>
where
    T: Send + Sync,
{
    let radius = (node.bounding_box.max() - node.bounding_box.min()).length() / 2.0;

    match projection {
        Projection::Perspective(perspective_projection) => {
            let Some(physical_target_size) = physical_target_size else {
                return None;
            };

            let center = octree_transform
                .affine()
                .transform_point3a(node.bounding_box.center);
            let camera_center = Into::<Vec3A>::into(camera_transform.translation());
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

/// Computes visible nodes of an octree, given a camera frustum and node's bounding boxes
/// using octree structure to accelerate things.
fn compute_visible_nodes<T>(
    octree: &Octree<T>,
    octree_transform: &GlobalTransform,
    camera_transform: &GlobalTransform,
    frustum: &Frustum,
    projection: &Projection,
    physical_target_size: &Option<UVec2>,
    visible_nodes: &mut Vec<VisibleOctreeNode>,
) -> ()
where
    T: Send + Sync + TypePath,
{
    let Some(root) = octree.root() else {
        return;
    };

    struct StackItem<'a, T>
    where
        T: Send + Sync + TypePath,
    {
        node: &'a OctreeNode<T>,
        completely_visible: bool,
        parent_index: Option<usize>,
    }

    let mut stack = VecDeque::from([StackItem {
        node: root,
        completely_visible: false,
        parent_index: None,
    }]);

    let world_from_local = octree_transform.affine();

    while let Some(StackItem {
        node,
        mut completely_visible,
        parent_index,
    }) = stack.pop_front()
    {
        let screen_pixel_radius = compute_screen_pixel_radius(
            node,
            octree_transform,
            camera_transform,
            projection,
            physical_target_size,
        );
        if let Some(screen_pixel_radius) = screen_pixel_radius {
            // TODO make this const as a parameter
            if screen_pixel_radius < 150.0 {
                // this node is too small to display it
                continue;
            }
        }

        if !completely_visible {
            let model_sphere = Sphere {
                center: world_from_local.transform_point3a(node.bounding_box.center),
                radius: octree_transform.radius_vec3a(node.bounding_box.half_extents),
            };

            // Do quick sphere-based frustum culling
            if !frustum.intersects_sphere(&model_sphere, false) {
                // this node is not visible, continue
                continue;
            }

            if frustum.contains_aabb(&node.bounding_box, &world_from_local) {
                // mark as completely visible to prevent later checks
                completely_visible = true;

                // else, do oriented bounding box frustum culling
            } else if !frustum.intersects_obb(&node.bounding_box, &world_from_local, true, false) {
                // the node is completely outside the frustum, ignore it
                continue;
            }
        }

        // we have to process child nodes, sending flag `completely_visible` to prevent useless visibility checks
        let current_index = visible_nodes.len();

        for i in iter_one_bits(node.children_mask) {
            let child = &node.children[i];
            let Some(child) = octree.get(*child) else {
                warn!("An octree node is missing in the hierarchy, shouldn't happen");
                continue;
            };
            stack.push_back(StackItem {
                node: child,
                completely_visible,
                parent_index: Some(current_index),
            });
        }

        // add the current node because it is visible or partially visible
        visible_nodes.push(node.into());

        // if there is a parent
        if let Some(parent_index) = parent_index {
            let parent = &mut visible_nodes[parent_index];

            // if there is no first child, set it
            if parent.first_child_index == 0 {
                parent.first_child_index = current_index;
            }
            // update the mask of visible children
            parent.children_mask |= 1 << node.child_index;
        }
    }

    // info!("Visible nodes: {}", visible_nodes.len());
}

/// This system extracts computed visible octree nodes and add them in the render world, for each view (camera)
pub fn extract_visible_octree_nodes<T, C>(
    mut commands: Commands,
    query: Extract<Query<(RenderEntity, &VisibleOctreeNodes<T, C>), With<Camera>>>,
    mapper: Extract<Query<&RenderEntity>>,
    point_cloud_octree_3ds: Extract<Query<&PointCloudOctree3d>>,
    mut render_octree_index: ResMut<RenderOctreeIndex<C>>,
) where
    T: Send + Sync + TypePath,
    C: Component,
{
    for (entity, visible_point_cloud_octree_3d_nodes) in query.iter() {
        let render_visible_point_cloud_octree_3d_nodes = RenderVisibleOctreeNodes::<T, C> {
            octrees: visible_point_cloud_octree_3d_nodes
                .octrees
                .clone()
                .into_iter()
                // for each visible octree, extract visible nodes, and store them using the render entity reference
                .map(|(entity, data)| {
                    let render_entity = mapper
                        .get(entity)
                        .expect("Render entity for PointCloudOctree3d not found");

                    let asset_id = point_cloud_octree_3ds
                        .get(entity)
                        .expect("Point cloud octree asset not found");

                    // makes sure an index exists for this entity
                    render_octree_index.add_octree(render_entity.id());

                    (render_entity.id(), data)
                })
                .collect(),
            _phantom_data: PhantomData,
        };
        commands
            .entity(entity)
            .insert(render_visible_point_cloud_octree_3d_nodes);
    }
}

/// This component stores the visible nodes for each octree at view level (camera) in "render world".
#[derive(Clone, Component, Default, Debug)]
pub struct RenderVisibleOctreeNodes<T, C>
where
    T: Send + Sync + TypePath,
    C: Component,
{
    /// The `Entity` used here refers to the "render world"
    pub octrees: HashMap<Entity, (AssetId<Octree<T>>, Vec<VisibleOctreeNode>)>,
    _phantom_data: PhantomData<C>,
}

impl<T, C> ExtractComponent for VisibleOctreeNodes<T, C>
where
    T: Send + Sync + TypePath,
    C: Component,
{
    type QueryData = &'static Self;
    type QueryFilter = With<Camera>;
    type Out = RenderVisibleOctreeNodes<T, C>;

    fn extract_component(
        visible_octree_nodes: QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        Some(RenderVisibleOctreeNodes::<T, C> {
            octrees: visible_octree_nodes.octrees.clone(),
            _phantom_data: PhantomData,
        })
    }
}

fn iter_one_bits(mask: u8) -> impl Iterator<Item = usize> {
    (0..8).filter(move |&i| (mask & (1 << i)) != 0)
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_iter_one_bits() {
        let mask: u8 = 0b01010101;
        let indexes = iter_one_bits(mask).collect::<Vec<_>>();
        assert_eq!(indexes, vec![0, 2, 4, 6]);
    }
}
