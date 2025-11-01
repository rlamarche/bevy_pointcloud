use core::any::TypeId;

use bevy_log::prelude::*;

use bevy_asset::Assets;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_reflect::{Reflect, std_traits::ReflectDefault};
use bevy_transform::components::GlobalTransform;

use crate::octree::asset::{NodeId, OctreeNode};
use crate::pointcloud_octree::asset::{PointCloudNodeData, PointCloudOctree};
use crate::pointcloud_octree::component::PointCloudOctree3d;
use bevy_camera::visibility::{
    NoCpuCulling, RenderLayers, VisibilityClass, VisibilityRange, VisibleEntities,
    VisibleEntityRanges,
};
use bevy_camera::{
    Camera, Projection,
    primitives::{Frustum, Sphere},
};
use bevy_math::UVec2;
use bevy_platform::collections::HashMap;

#[derive(Clone, Component, Default, Debug, Reflect)]
#[reflect(Component, Default, Debug, Clone)]
pub struct VisibleOctreeNodes {
    #[reflect(ignore, clone)]
    pub nodes: HashMap<Entity, Vec<NodeId>>,
}

impl VisibleOctreeNodes {
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
pub fn check_octree_node_visibility(
    mut view_query: Query<(
        Entity,
        &VisibleEntities,
        &Frustum,
        &Camera,
        &GlobalTransform,
        &Projection,
        &mut VisibleOctreeNodes,
        Has<NoCpuCulling>,
    )>,
    pointcloud_octrees_3d: Query<(&PointCloudOctree3d, &GlobalTransform)>,
    pointcloud_octrees: Res<Assets<PointCloudOctree>>,
) {
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

        let visible_pointcloud_octrees_3d =
            visible_entities.get(TypeId::of::<PointCloudOctree3d>());

        for entity in visible_pointcloud_octrees_3d {
            let (pointcloud_octree_3d, octree_transform) = match pointcloud_octrees_3d.get(*entity)
            {
                Ok(item) => item,
                Err(error) => {
                    warn!("Missing point cloud 3d entity in query: {:#}", error);
                    continue;
                }
            };
            let Some(pointcloud_octree) = pointcloud_octrees.get(pointcloud_octree_3d) else {
                warn!(
                    "Missing point cloud octree in assets: {:?}",
                    pointcloud_octree_3d
                );
                continue;
            };

            info!("Computing visible nodes");
            // compute visible nodes and store them
            compute_visible_nodes(
                pointcloud_octree,
                octree_transform,
                camera_transform,
                frustum,
                projection,
                &physical_target_size,
                visible_octree_nodes.get_mut(*entity),
            );
            info!("{} visible nodes", visible_octree_nodes.get(*entity).len());
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

fn compute_screen_pixel_radius<'a>(
    node: &OctreeNode<PointCloudNodeData>,
    octree_transform: &GlobalTransform,
    camera_transform: &GlobalTransform,
    projection: &Projection,
    physical_target_size: &Option<UVec2>,
) -> Option<f32> {
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

fn compute_visible_nodes(
    octree: &PointCloudOctree,
    octree_transform: &GlobalTransform,
    camera_transform: &GlobalTransform,
    frustum: &Frustum,
    projection: &Projection,
    physical_target_size: &Option<UVec2>,
    visible_nodes: &mut Vec<NodeId>,
) -> () {
    let Some(root) = octree.root() else {
        return;
    };
    
    let mut stack = vec![(root, false)];
    // let mut nodes_to_load = Vec::new();

    let world_from_local = octree_transform.affine();

    while let Some((node, mut completely_visible)) = stack.pop() {
        // get the current node future index
        let current_index = visible_nodes.len();

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
        for child in &node.children {
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
