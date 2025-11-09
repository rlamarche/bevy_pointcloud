use crate::octree::asset::{NodeId, Octree, OctreeNode};
use crate::octree::visibility::extract::{
    ExtractOctreeNode, RenderOctreeNodes, extract_render_octree_nodes,
};
use bevy_app::{App, Plugin, PostUpdate, SubApp};
use bevy_asset::{Asset, AssetId, Assets};
use bevy_camera::visibility::{
    NoCpuCulling, RenderLayers, Visibility, VisibilityClass, VisibilityRange, VisibleEntities,
    VisibleEntityRanges, add_visibility_class, check_visibility,
};
use bevy_camera::{
    Camera, Projection,
    primitives::{Frustum, Sphere},
};
use bevy_ecs::bundle::NoBundleEffect;
use bevy_ecs::prelude::*;
use bevy_ecs::query::{QueryFilter, QueryItem, ReadOnlyQueryData};
use bevy_ecs::schedule::ScheduleConfigs;
use bevy_ecs::system::ScheduleSystem;
use bevy_log::prelude::*;
use bevy_math::UVec2;
use bevy_math::prelude::*;
use bevy_platform::collections::HashMap;
use bevy_reflect::{Reflect, TypePath, std_traits::ReflectDefault};
use bevy_render::camera::extract_cameras;
use bevy_render::extract_component::ExtractComponent;
use bevy_render::sync_world::RenderEntity;
use bevy_render::{Extract, ExtractSchedule, Render, RenderApp, RenderSystems};
use bevy_transform::components::GlobalTransform;
use core::any::TypeId;
use prepare::{PrepareNextFrameOctreeNodes, RenderOctreeNode, RenderOctrees, prepare_assets};
use prepare::{
    RenderAssetBytesPerFrame, RenderAssetBytesPerFrameLimiter,
    extract_render_asset_bytes_per_frame, reset_render_asset_bytes_per_frame,
};
use std::fmt::Debug;
use std::marker::PhantomData;

pub mod extract;
pub mod prepare;

pub struct ExtractVisibleOctreeNodesPlugin<T, C, A, AFTER = ()>(
    PhantomData<fn() -> (T, C, A, AFTER)>,
)
where
    T: ExtractOctreeNode + Send + Sync + TypePath,
    C: Component,
    for<'a> &'a C: Into<AssetId<Octree<T>>>,
    A: RenderOctreeNode<ExtractedOctreeNode = T::Out>,
    AFTER: RenderOctreeDependency + 'static;

impl<T, C, A, AFTER> Default for ExtractVisibleOctreeNodesPlugin<T, C, A, AFTER>
where
    T: ExtractOctreeNode + Send + Sync + TypePath,
    C: Component,
    for<'a> &'a C: Into<AssetId<Octree<T>>>,
    A: RenderOctreeNode<ExtractedOctreeNode = T::Out>,
    AFTER: RenderOctreeDependency + 'static,
{
    fn default() -> Self {
        ExtractVisibleOctreeNodesPlugin(PhantomData)
    }
}

impl<T, C, A, AFTER> Plugin for ExtractVisibleOctreeNodesPlugin<T, C, A, AFTER>
where
    T: ExtractOctreeNode + Send + Sync + TypePath,
    C: Component,
    for<'a> &'a C: Into<AssetId<Octree<T>>>,
    A: RenderOctreeNode<ExtractedOctreeNode = T::Out>,
    AFTER: RenderOctreeDependency + 'static,
{
    fn build(&self, app: &mut App) {
        app.init_resource::<RenderAssetBytesPerFrame>();

        app.register_required_components::<C, Visibility>()
            .register_required_components::<C, VisibilityClass>()
            .register_required_components::<Camera, VisibleOctreeNodes<T>>()
            .add_systems(
                PostUpdate,
                check_octree_node_visibility::<C, T>.after(check_visibility),
            );

        app.world_mut()
            .register_component_hooks::<C>()
            .on_add(add_visibility_class::<C>);

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.init_resource::<RenderAssetBytesPerFrameLimiter>();
        render_app
            .add_systems(ExtractSchedule, extract_render_asset_bytes_per_frame)
            .add_systems(
                Render,
                reset_render_asset_bytes_per_frame.in_set(RenderSystems::Cleanup),
            );

        render_app
            .init_resource::<RenderOctreeNodes<T>>()
            .init_resource::<RenderOctrees<A>>()
            .init_resource::<PrepareNextFrameOctreeNodes<A>>()
            .add_systems(
                ExtractSchedule,
                (
                    extract_visible_octree_nodes::<T>.after(extract_cameras),
                    extract_render_octree_nodes::<T, C>.after(extract_visible_octree_nodes::<T>),
                ),
            );

        AFTER::register_system(
            render_app,
            prepare_assets::<A>.in_set(RenderSystems::PrepareAssets),
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

#[derive(Clone, Component, Debug)]
pub struct VisibleOctreeNodes<T>
where
    T: Send + Sync + TypePath,
{
    pub nodes: HashMap<Entity, Vec<NodeId>>,
    phantom_data: PhantomData<T>,
}

impl<T> Default for VisibleOctreeNodes<T>
where
    T: Send + Sync + TypePath,
{
    fn default() -> Self {
        Self {
            nodes: HashMap::default(),
            phantom_data: PhantomData,
        }
    }
}

impl<T> VisibleOctreeNodes<T>
where
    T: Send + Sync + TypePath,
{
    pub fn get(&self, entity: Entity) -> &[NodeId] {
        match self.nodes.get(&entity) {
            Some(entities) => &entities[..],
            None => &[],
        }
    }

    pub fn get_mut(&mut self, entity: Entity) -> &mut Vec<NodeId> {
        self.nodes.entry(entity).or_default()
    }

    pub fn iter(&self, entity: Entity) -> impl DoubleEndedIterator<Item = &NodeId> {
        self.get(entity).iter()
    }

    pub fn len(&self, entity: Entity) -> usize {
        self.get(entity).len()
    }

    pub fn is_empty(&self, entity: Entity) -> bool {
        self.get(entity).is_empty()
    }

    pub fn clear(&mut self, entity: Entity) {
        self.get_mut(entity).clear();
    }

    pub fn clear_all(&mut self) {
        // Don't just nuke the hash table; we want to reuse allocations.
        for nodes in self.nodes.values_mut() {
            nodes.clear();
        }
    }

    pub fn push(&mut self, node_id: NodeId, entity: Entity) {
        self.get_mut(entity).push(node_id);
    }
}

/// System updating the visibility of entities each frame.
///
/// The system is part of the [`VisibilitySystems::CheckVisibility`] set. Each
/// frame, it updates the [`ViewVisibility`] of all entities, and for each view
/// also compute the [`VisibleEntities`] for that view.
///
/// To ensure that an entity is checked for visibility, make sure that it has a
/// [`VisibilityClass`] component and that that component is nonempty.
pub fn check_octree_node_visibility<C, T>(
    mut view_query: Query<(
        Entity,
        &VisibleEntities,
        &Frustum,
        &Camera,
        &GlobalTransform,
        &Projection,
        &mut VisibleOctreeNodes<T>,
        Has<NoCpuCulling>,
    )>,
    octree_entities: Query<(&C, &GlobalTransform)>,
    pointcloud_octrees: Res<Assets<Octree<T>>>,
) where
    C: Component,
    T: Send + Sync + TypePath,
    for<'a> &'a C: Into<AssetId<Octree<T>>>,
{
    for (
        view,
        visible_entities,
        frustum,
        camera,
        camera_transform,
        projection,
        mut visible_octree_nodes,
        no_cpu_culling,
    ) in &mut view_query
    {
        if !camera.is_active {
            continue;
        }

        // clear all previous visible nodes
        visible_octree_nodes.clear_all();

        let physical_target_size = camera.physical_target_size();

        let visible_octree_entities = visible_entities.get(TypeId::of::<C>());

        for entity in visible_octree_entities {
            let (octree_entity, octree_transform) = match octree_entities.get(*entity) {
                Ok(item) => item,
                Err(error) => {
                    warn!("Missing octree entity in query: {:#}", error);
                    continue;
                }
            };
            // let Some(octree) = pointcloud_octrees.get(Into::<AssetId<Octree<T>>>::into(octree_entity))
            let Some(octree) = pointcloud_octrees.get(octree_entity.into()) else {
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
                visible_octree_nodes.get_mut(*entity),
            );
        }
    }
}

#[derive(Clone, Debug)]
pub struct CameraView {
    pub global_transform: GlobalTransform,
    pub frustum: Frustum,
    pub projection: Projection,
    pub physical_target_size: Option<UVec2>,
}

fn compute_screen_pixel_radius<'a, T>(
    node: &OctreeNode<T>,
    octree_transform: &GlobalTransform,
    camera_transform: &GlobalTransform,
    projection: &Projection,
    physical_target_size: &Option<UVec2>,
) -> Option<f32>
where
    T: Send + Sync + TypePath,
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

fn compute_visible_nodes<T>(
    octree: &Octree<T>,
    octree_transform: &GlobalTransform,
    camera_transform: &GlobalTransform,
    frustum: &Frustum,
    projection: &Projection,
    physical_target_size: &Option<UVec2>,
    visible_nodes: &mut Vec<NodeId>,
) -> ()
where
    T: Send + Sync + TypePath,
{
    let Some(root) = octree.root() else {
        return;
    };

    let mut stack = vec![(root, false)];
    // let mut nodes_to_load = Vec::new();

    let world_from_local = octree_transform.affine();

    while let Some((node, mut completely_visible)) = stack.pop() {
        let screen_pixel_radius = compute_screen_pixel_radius(
            node,
            octree_transform,
            camera_transform,
            projection,
            physical_target_size,
        );
        if let Some(screen_pixel_radius) = screen_pixel_radius {
            if screen_pixel_radius < 300.0 {
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

        // if node.node_type == 2 {
        //     // the node has a child hierarchy, plan to load it
        //     nodes_to_load.push(node.id.expect("node should have a node id"));
        // }

        // we have to process child nodes, sending flag `completely_visible` to prevent useless visibility checks
        for i in iter_zero_bits(node.children_mask) {
            let child = &node.children[i];
            let Some(child) = octree.get(*child) else {
                warn!("missing node in hierarchy, shouldn't happen");
                continue;
            };
            stack.push((child, completely_visible));
        }

        // add the current node because it is visible or partially visible
        visible_nodes.push(node.id);
    }
}

fn extract_visible_octree_nodes<T>(
    mut commands: Commands,
    query: Extract<Query<(Entity, RenderEntity, &Camera, &VisibleOctreeNodes<T>)>>,
    mapper: Extract<Query<&RenderEntity>>,
) where
    T: Send + Sync + TypePath,
{
    for (_entity, render_entity, camera, visible_point_cloud_octree_3d_nodes) in query.iter() {
        let render_visible_point_cloud_octree_3d_nodes = RenderVisibleOctreeNodes {
            octrees: visible_point_cloud_octree_3d_nodes
                .nodes
                .clone()
                .into_iter()
                .map(|(entity, nodes)| {
                    let render_entity = mapper
                        .get(entity)
                        .expect("Render entity for PointCloudOctree3d not found");
                    (render_entity.id(), nodes)
                })
                .collect(),
        };
        commands
            .entity(render_entity)
            .insert(render_visible_point_cloud_octree_3d_nodes);
    }
}

fn iter_zero_bits(mask: u8) -> impl Iterator<Item = usize> {
    (0..8).filter(move |&i| (mask & (1 << i)) == 0)
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_iter_zero_bits() {
        let mask: u8 = 0b01010101;
        let indexes = iter_zero_bits(mask).collect::<Vec<_>>();
        assert_eq!(indexes, vec![1, 3, 5, 7]);
    }
}

#[derive(Clone, Component, Default, Debug, Reflect)]
#[reflect(Component, Default, Debug, Clone)]
pub struct RenderVisibleOctreeNodes {
    #[reflect(ignore, clone)]
    pub octrees: HashMap<Entity, Vec<NodeId>>,
}

impl<T> ExtractComponent for VisibleOctreeNodes<T>
where
    T: Send + Sync + TypePath,
{
    type QueryData = &'static Self;
    type QueryFilter = With<Camera>;
    type Out = RenderVisibleOctreeNodes;

    fn extract_component(
        (visible_octree_nodes): QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        Some(RenderVisibleOctreeNodes {
            octrees: visible_octree_nodes.nodes.clone(),
        })
    }
}
