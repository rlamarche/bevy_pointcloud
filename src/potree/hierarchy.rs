use std::collections::VecDeque;
use super::asset::PotreePointCloud;
use super::point_cloud::{PotreeMainCamera, PotreePointCloud3d};
use super::spawn_async_task::spawn_async_task;
use async_lock::RwLock;
use bevy_asset::prelude::*;
use bevy_camera::prelude::{Camera, Projection};
use bevy_camera::primitives::{Aabb, Frustum};
use bevy_ecs::prelude::*;
use bevy_log::prelude::*;
use bevy_math::prelude::*;
use bevy_math::{Vec3, Vec3A};
use bevy_transform::prelude::*;
use crossbeam::queue::ArrayQueue;
use potree::octree::node::{OctreeNode, iter_one_bits, NodeType};
use potree::octree::{FlatOctree, NodeId};
use potree::prelude::OctreeNodeSnapshot;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct CameraView {
    pub global_transform: GlobalTransform,
    pub frustum: Frustum,
    pub projection: Projection,
    pub physical_target_size: Option<UVec2>,
}

/// Component to communicate with the update hierarchy long running task
#[derive(Component)]
pub struct HierarchyTask {
    pub wakeup_tx: async_channel::Sender<()>,
    pub hierarchy_snapshot_rx: async_channel::Receiver<Vec<OctreeNodeSnapshot>>,
    pub camera_view_queue: Arc<ArrayQueue<CameraView>>,
    #[cfg(not(feature = "potree_wasm_worker"))]
    pub task: bevy_tasks::Task<()>,
}

#[derive(Component)]
pub struct HierarchySnapshot(pub Vec<OctreeNodeSnapshot>);

pub fn init_hierarchy_task(
    mut commands: Commands,
    potree_point_clouds: Res<Assets<PotreePointCloud>>,
    potree_point_clouds_3d: Query<
        (Entity, &PotreePointCloud3d, &GlobalTransform),
        Without<HierarchyTask>,
    >,
) {
    for (entity, potree_point_cloud_3d, global_transform) in potree_point_clouds_3d {
        let (wakeup_tx, wakeup_rx) = async_channel::bounded(1);
        let (hierarchy_snapshot_tx, hierarchy_snapshot_rx) = async_channel::bounded(1);

        let camera_view_queue = Arc::new(crossbeam::queue::ArrayQueue::new(1));

        let Some(potree_point_cloud) = potree_point_clouds.get(&potree_point_cloud_3d.handle)
        else {
            continue;
        };

        #[allow(unused)]
        let task = spawn_async_task({
            let update_hierarchy_task = UpdateHierarchyTask {
                global_transform: global_transform.clone(),
                wakeup_rx,
                camera_view_queue: camera_view_queue.clone(),
                hierarchy: potree_point_cloud.hierarchy.clone(),
                hierarchy_snapshot_tx,
            };
            update_hierarchy_task.run()
        });

        commands.entity(entity).insert(HierarchyTask {
            wakeup_tx,
            hierarchy_snapshot_rx,
            camera_view_queue,
            #[cfg(not(feature = "potree_wasm_worker"))]
            task,
        });
    }
}

pub fn update_hierarchy(
    mut commands: Commands,
    frustum: Query<(&Camera, &Frustum, &GlobalTransform, &Projection), With<PotreeMainCamera>>,
    _potree_point_clouds: Res<Assets<PotreePointCloud>>,
    potree_point_clouds_3d: Query<
        (
            Entity,
            &HierarchyTask,
            &PotreePointCloud3d,
            &GlobalTransform,
        ),
        With<HierarchyTask>,
    >,
) {
    let Ok((camera, frustum, camera_global_transform, projection)) = frustum.single() else {
        return;
    };

    let camera_view = CameraView {
        frustum: frustum.clone(),
        global_transform: camera_global_transform.clone(),
        projection: projection.clone(),
        physical_target_size: camera.physical_target_size(),
    };

    for (entity, hierarchy_task, potree_point_cloud_3d, global_transform) in potree_point_clouds_3d
    {
        hierarchy_task
            .camera_view_queue
            .force_push(camera_view.clone());
        let _ = hierarchy_task.wakeup_tx.try_send(());

        if let Ok(hierarchy_snapshot) = hierarchy_task.hierarchy_snapshot_rx.try_recv() {
            commands
                .entity(entity)
                .insert(HierarchySnapshot(hierarchy_snapshot));
        }
    }
}

pub struct UpdateHierarchyTask {
    pub global_transform: GlobalTransform,
    pub wakeup_rx: async_channel::Receiver<()>,
    pub camera_view_queue: Arc<ArrayQueue<CameraView>>,
    pub hierarchy: Arc<RwLock<potree::hierarchy::Hierarchy>>,
    pub hierarchy_snapshot_tx: async_channel::Sender<Vec<OctreeNodeSnapshot>>,
}

impl UpdateHierarchyTask {
    pub async fn run(mut self) {
        // load initial hierarchy at start
        // self.point_cloud
        //     .write()
        //     .await
        //     .load_entire_hierarchy()
        //     .await
        //     .expect("unable to load entire hierarchy");

        let mut prev_camera_view: Option<CameraView> = None;
        // Watch if we need to wakeup
        while let Ok(_) = self.wakeup_rx.recv().await {
            if let Some(camera_view) = self.camera_view_queue.pop() {
                if let Some(prev_camera_view) = &prev_camera_view {
                    if frustum_equals(&prev_camera_view.frustum, &camera_view.frustum) {
                        // if the frustum has not changed, skip it
                        continue;
                    }
                }

                self.process_camera_view(&camera_view).await;

                let _ = prev_camera_view.insert(camera_view);
            }
        }
    }

    async fn process_camera_view(&self, camera_view: &CameraView) {
        let mut point_cloud = self.hierarchy.write().await;

        let octree = point_cloud.octree();
        let root = octree.root();

        let (visible_nodes, nodes_to_load) =
            self.compute_visible_nodes(octree, root, &self.global_transform, camera_view);

        // load node proxies
        for node_id in nodes_to_load {
            // info!("Loading sub node hierarchy for node {}", node_id);
            if let Err(error) = point_cloud.load_hierarchy(node_id).await {
                warn!("Unable to load sub node hierarchy: {:#}", error);
            }
        }

        // send visible nodes to main
        if let Err(error) = self.hierarchy_snapshot_tx.force_send(visible_nodes) {
            warn!(
                "Failed to send hierarchy snapshot to point cloud: {:#}",
                error
            );
        }
    }

    fn compute_screen_pixel_radius<'a>(
        &self,
        node: &'a OctreeNode,
        transform: &GlobalTransform,
        camera_view: &CameraView,
    ) -> Option<f32> {
        let min = node.bounding_box.min;
        let max = node.bounding_box.max;

        let model_aabb = Aabb::from_min_max(
            Vec3::new(min.x as f32, min.y as f32, min.z as f32),
            Vec3::new(max.x as f32, max.y as f32, max.z as f32),
        );

        let radius = (model_aabb.max() - model_aabb.min()).length() / 2.0;

        match &camera_view.projection {
            Projection::Perspective(perspective_projection) => {
                let Some(physical_target_size) = &camera_view.physical_target_size else {
                    return None;
                };

                let center = transform.affine().transform_point3a(model_aabb.center);
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

    fn compute_visible_nodes<'a>(
        &self,
        octree: &'a FlatOctree<OctreeNode>,
        node: &'a OctreeNode,
        transform: &GlobalTransform,
        camera_view: &CameraView,
    ) -> (Vec<OctreeNodeSnapshot>, Vec<NodeId>) {
        let mut stack = VecDeque::from([(0_usize, node, false)]);
        let mut visible_nodes = Vec::<OctreeNodeSnapshot>::new();
        let mut nodes_to_load = Vec::new();

        let world_from_local = transform.affine();

        while let Some((parent_index, node, mut completely_visible)) = stack.pop_front() {
            if node.node_type.has_points() && node.num_points == 0 {
                // skip nodes with no points
                continue;
            }

            // get the current node future index
            let current_index = visible_nodes.len();

            let screen_pixel_radius =
                self.compute_screen_pixel_radius(node, transform, camera_view);
            if let Some(screen_pixel_radius) = screen_pixel_radius {
                if screen_pixel_radius < 150.0 {
                    // this node is too small to display it
                    continue;
                }
            }

            if !completely_visible {
                let min = node.bounding_box.min;
                let max = node.bounding_box.max;

                let model_aabb = Aabb::from_min_max(
                    Vec3::new(min.x as f32, min.y as f32, min.z as f32),
                    Vec3::new(max.x as f32, max.y as f32, max.z as f32),
                );
                let model_sphere = bevy_camera::primitives::Sphere {
                    center: world_from_local.transform_point3a(model_aabb.center),
                    radius: transform.radius_vec3a(model_aabb.half_extents),
                };

                // Do quick sphere-based frustum culling
                if !camera_view.frustum.intersects_sphere(&model_sphere, false) {
                    // this node is not visible, continue
                    continue;
                }

                if camera_view
                    .frustum
                    .contains_aabb(&model_aabb, &world_from_local)
                {
                    // mark as completely visible to prevent later checks
                    completely_visible = true;

                    // else, do oriented bounding box frustum culling
                } else if !camera_view.frustum.intersects_obb(
                    &model_aabb,
                    &world_from_local,
                    true,
                    false,
                ) {
                    // the node is completely outside the frustum, ignore it
                    continue;
                }
            }

            if matches!(node.node_type, NodeType::Proxy) {
                // the node has a child hierarchy, plan to load it
                nodes_to_load.push(node.id.expect("node should have a node id"));
            } else {
                // we have to process child nodes, sending flag `completely_visible` to prevent useless visibility checks
                for i in iter_one_bits(node.children_mask) {
                    let child = &node.children[i as usize];
                    let child = octree
                        .node(*child)
                        .expect("missing node in hierarchy, shouldn't happen");
                    stack.push_back((current_index, child, completely_visible));
                }

                // add the current node because it is visible or partially visible
                let mut node_snapshot: OctreeNodeSnapshot = node.into();
                // don't forget to set its index
                node_snapshot.index = current_index;
                let child_index = node_snapshot.child_index;
                visible_nodes.push(node_snapshot);

                // if there is a parent, add it to the children array on an empty space
                if parent_index < current_index {
                    let parent = &mut visible_nodes[parent_index];
                    parent.children[child_index as usize] = current_index;
                    parent.children_mask |= 1 << node.child_index;
                }
            }
        }

        (visible_nodes, nodes_to_load)
    }
}

fn frustum_equals(a: &Frustum, b: &Frustum) -> bool {
    for i in 0..6 {
        if a.half_spaces[i].normal_d().eq(&b.half_spaces[i].normal_d()) {
            continue;
        } else {
            return false;
        }
    }
    true
}
