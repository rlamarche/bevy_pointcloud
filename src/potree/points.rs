use super::asset::PotreePointCloud;
use super::point_cloud::PotreePointCloud3d;
use super::spawn_async_task::spawn_async_task;
use crate::point_cloud::{PointCloud, PointCloud3d, PointCloudData};
use crate::point_cloud_material::{PointCloudMaterial, PointCloudMaterial3d};
use crate::pointcloud_octree::asset::PointCloudOctree;
use crate::pointcloud_octree::component::PointCloudOctree3d;
use crate::pointcloud_octree::potree::mapping::PotreePointCloudOctreeNodes;
use crate::potree::hierarchy::HierarchySnapshot;
use async_lock::RwLock;
use bevy_asset::prelude::*;
use bevy_camera::primitives::Aabb;
use bevy_ecs::prelude::*;
use bevy_gizmos::prelude::*;
use bevy_log::prelude::*;
use bevy_math::Vec3;
use bevy_platform::collections::HashSet;
use bevy_transform::prelude::*;
use potree::prelude::OctreeNodeSnapshot;
use potree::{octree::NodeId as PotreeNodeId, prelude::PointData as PotreePointData};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct LoadPointsMessage {
    pub node: OctreeNodeSnapshot,
}

#[derive(Clone, Debug)]
pub struct LoadedPointsMessage {
    pub node: OctreeNodeSnapshot,
    pub points: Vec<PotreePointData>,
}

/// Component to communicate with the update hierarchy long running task
#[derive(Component)]
pub struct LoadPointsTaskHolder {
    pub loaded_points_rx: async_channel::Receiver<LoadedPointsMessage>,
    pub load_points_queue_tx: async_channel::Sender<LoadPointsMessage>,
    pub loaded_points: HashSet<PotreeNodeId>,
    #[cfg(not(feature = "potree_wasm_worker"))]
    pub task: bevy_tasks::Task<()>,
}

pub fn init_load_points_task(
    mut commands: Commands,
    potree_point_clouds: Res<Assets<PotreePointCloud>>,
    potree_point_clouds_3d: Query<
        (Entity, &PotreePointCloud3d, &Gizmo, &GlobalTransform),
        Without<LoadPointsTaskHolder>,
    >,
) {
    for (entity, potree_point_cloud_3d, gizmo, global_transform) in potree_point_clouds_3d {
        let (loaded_points_tx, loaded_points_rx) = async_channel::unbounded();
        let (load_points_queue_tx, load_points_queue_rx) = async_channel::unbounded();

        let Some(potree_point_cloud) = potree_point_clouds.get(&potree_point_cloud_3d.handle)
        else {
            continue;
        };

        let task = spawn_async_task({
            let load_points_task = LoadPointsTask {
                load_points_queue_rx,
                point_cloud: potree_point_cloud.hierarchy.clone(),
                loaded_points_tx,
            };
            load_points_task.run()
        });

        commands.entity(entity).insert(LoadPointsTaskHolder {
            loaded_points_rx,
            load_points_queue_tx,
            loaded_points: HashSet::new(),
            #[cfg(not(feature = "potree_wasm_worker"))]
            task,
        });
    }
}

pub fn load_points_tx(
    potree_point_clouds_3d: Query<
        (&mut LoadPointsTaskHolder, &HierarchySnapshot),
        With<LoadPointsTaskHolder>,
    >,
) {
    for (mut load_points_task_holder, hierarchy_snapshot) in potree_point_clouds_3d {
        for octree_node in &hierarchy_snapshot.0 {
            let node_id = octree_node.id.expect("missing node id");

            if !load_points_task_holder.loaded_points.contains(&node_id) {
                if let Ok(_) =
                    load_points_task_holder
                        .load_points_queue_tx
                        .try_send(LoadPointsMessage {
                            node: octree_node.clone(),
                        })
                {
                    load_points_task_holder.loaded_points.insert(node_id);
                }
            }
        }
    }
}

pub fn load_points_rx(
    potrees: Res<Assets<PotreePointCloud>>,
    mut point_cloud_octrees: ResMut<Assets<PointCloudOctree>>,
    potree_point_clouds_3d: Query<
        (
            Entity,
            &mut LoadPointsTaskHolder,
            &HierarchySnapshot,
            &PotreePointCloud3d,
            &PointCloudOctree3d,
        ),
        With<LoadPointsTaskHolder>,
    >,
    mut potree_point_cloud_octree_nodes: ResMut<PotreePointCloudOctreeNodes>,
) {
    for (
        entity,
        mut load_points_task_holder,
        hierarchy_snapshot,
        potree_point_cloud_3d,
        point_cloud_octree_3d,
    ) in potree_point_clouds_3d
    {
        let Some(potree) = potrees.get(potree_point_cloud_3d) else {
            warn!("Missing potree");
            continue;
        };

        let Some(mut point_cloud_octree) = point_cloud_octrees.get_mut(point_cloud_octree_3d)
        else {
            warn!("Missing octree");
            continue;
        };

        let mut mapping = potree_point_cloud_octree_nodes.get_or_insert_mut(point_cloud_octree_3d);

        while let Ok(loaded_points) = load_points_task_holder.loaded_points_rx.try_recv() {
            let Some(potree_node_id) = loaded_points.node.id else {
                continue;
            };

            let parent_id = loaded_points.node.parent_id.map(|parent_id| {
                mapping
                    .get_octree_node_id(parent_id)
                    .expect("missing node in potree:octree mapping")
            });

            let node_id = point_cloud_octree
                .insert(
                    parent_id,
                    Aabb::from_min_max(
                        loaded_points.node.bounding_box.min.as_vec3(),
                        loaded_points.node.bounding_box.max.as_vec3(),
                    ),
                    (&loaded_points.node, loaded_points.points).into(),
                )
                .expect("Unable to insert loaded points in octree");

            mapping.insert(potree_node_id, node_id);

            // commands.spawn((
            //     PointCloud3d(point_cloud_handle),
            //     PointCloudMaterial3d(point_cloud_material_handle),
            //     transform.clone(),
            // ));
        }

        // while let Ok(loaded_points) = load_points_task_holder.loaded_points_rx.try_recv() {
        //     let point_cloud_material_handle = point_cloud_materials.add(PointCloudMaterial {
        //         point_size: 30.0,
        //         ..Default::default()
        //     });
        //
        //     commands.spawn((
        //         PointCloud3d(point_cloud_handle),
        //         PointCloudMaterial3d(point_cloud_material_handle),
        //         transform.clone(),
        //     ));
        // }
    }
}

pub struct LoadPointsTask {
    pub load_points_queue_rx: async_channel::Receiver<LoadPointsMessage>,
    pub point_cloud: Arc<RwLock<potree::hierarchy::Hierarchy>>,
    pub loaded_points_tx: async_channel::Sender<LoadedPointsMessage>,
}

impl LoadPointsTask {
    pub async fn run(mut self) {
        while let Ok(load_points) = self.load_points_queue_rx.recv().await {
            if let Err(message) = self.process_message(load_points).await {
                error!("Failed to process load_points message: {}", message);
            }
        }
    }

    async fn process_message(&self, message: LoadPointsMessage) -> Result<(), String> {
        let mut point_cloud = self.point_cloud.read().await;

        let node_id = message.node.id.expect("missing node id");

        let points = point_cloud
            .load_points(message.node.id.expect("missing node id"))
            .await
            .map_err(|e| e.to_string())?;

        drop(point_cloud);

        // let points = points
        //     .into_iter()
        //     .map(|point_data| PointCloudData {
        //         position: Vec3::new(
        //             point_data.position.x as f32,
        //             point_data.position.y as f32,
        //             point_data.position.z as f32,
        //         ),
        //         point_size: -1.0,
        //         color: [
        //             point_data.color[0] as f32 / 256.0,
        //             point_data.color[1] as f32 / 256.0,
        //             point_data.color[2] as f32 / 256.0,
        //             1.0,
        //         ],
        //     })
        //     .collect::<Vec<_>>();

        // let point_cloud = PointCloud { points };

        self.loaded_points_tx
            .send(LoadedPointsMessage {
                node: message.node,
                points,
            })
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}
