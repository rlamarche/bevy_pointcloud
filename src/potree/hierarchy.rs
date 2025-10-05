use super::asset::PotreePointCloud;
use super::point_cloud::{PotreeMainCamera, PotreePointCloud3d};
use async_lock::RwLock;
use bevy_asset::prelude::*;
use bevy_camera::primitives::Frustum;
use bevy_ecs::prelude::*;
use bevy_gizmos::prelude::*;
use bevy_log::prelude::*;
use bevy_tasks::prelude::*;
use bevy_transform::prelude::*;
use crossbeam::queue::ArrayQueue;
use futures::SinkExt;
use std::sync::Arc;
use potree::prelude::OctreeNodeSnapshot;

/// Component to communicate with the update hierarchy long running task
#[derive(Component)]
pub struct HierarchyTask {
    pub wakeup_tx: async_channel::Sender<()>,
    pub hierarchy_snapshot_rx: async_channel::Receiver<OctreeNodeSnapshot>,
    pub frustum_queue: Arc<ArrayQueue<Frustum>>,
    #[cfg(not(feature = "potree_wasm_worker"))]
    pub task: bevy_tasks::Task<()>,
}

#[derive(Component)]
pub struct HierarchySnapshot(pub OctreeNodeSnapshot);

pub fn init_hierarchy_task(
    mut commands: Commands,
    potree_point_clouds: Res<Assets<PotreePointCloud>>,
    potree_point_clouds_3d: Query<
        (Entity, &PotreePointCloud3d, &Gizmo, &GlobalTransform),
        Without<HierarchyTask>,
    >,
) {
    let async_task_pool = AsyncComputeTaskPool::get();

    for (entity, potree_point_cloud_3d, gizmo, global_transform) in potree_point_clouds_3d {
        let (wakeup_tx, wakeup_rx) = async_channel::bounded(1);
        let (hierarchy_snapshot_tx, hierarchy_snapshot_rx) = async_channel::bounded(1);

        let frustum_queue = Arc::new(crossbeam::queue::ArrayQueue::<Frustum>::new(1));

        let Some(potree_point_cloud) = potree_point_clouds.get(&potree_point_cloud_3d.handle)
        else {
            continue;
        };

        let task = spawn_async_task(async_task_pool, {
            let update_hierarchy_task = UpdateHierarchyTask {
                wakeup_rx,
                frustum_queue: frustum_queue.clone(),
                point_cloud: potree_point_cloud.wrapped.clone(),
                hierarchy_snapshot_tx,
            };
            // let frustum_queue = frustum_queue.clone();
            update_hierarchy_task.run()
        });

        // let task = async_task_pool.spawn({
        //     let update_hierarchy_task = UpdateHierarchyTask {
        //         wakeup_rx,
        //         frustum_queue: frustum_queue.clone(),
        //         point_cloud: potree_point_cloud.wrapped.clone(),
        //     };
        //     // let frustum_queue = frustum_queue.clone();
        //     update_hierarchy_task.run()
        // });

        commands.entity(entity).insert(HierarchyTask {
            wakeup_tx,
            hierarchy_snapshot_rx,
            frustum_queue,
            #[cfg(not(feature = "potree_wasm_worker"))]
            task,
        });
    }
}

pub fn update_hierarchy(
    mut commands: Commands,
    frustum: Query<&Frustum, With<PotreeMainCamera>>,
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
    let Ok(frustum) = frustum.single() else {
        return;
    };

    for (entity, hierarchy_task, potree_point_cloud_3d, global_transform) in potree_point_clouds_3d
    {
        hierarchy_task.frustum_queue.force_push(frustum.clone());
        let _ = hierarchy_task.wakeup_tx.try_send(());

        if let Ok(hierarchy_snapshot) = hierarchy_task.hierarchy_snapshot_rx.try_recv() {
            commands
                .entity(entity)
                .insert(HierarchySnapshot(hierarchy_snapshot));
        }
    }
}

#[cfg(not(feature = "potree_wasm_worker"))]
fn spawn_async_task<T>(
    async_task_pool: &AsyncComputeTaskPool,
    future: impl Future<Output = T> + Send + 'static,
) -> bevy_tasks::Task<T>
where
    T: Send + 'static,
{
    async_task_pool.spawn(future)
}

#[cfg(feature = "potree_wasm_worker")]
fn spawn_async_task(
    async_task_pool: &AsyncComputeTaskPool,
    future: impl Future<Output = ()> + Send + 'static,
) -> () {
    wasm_thread::spawn({
        || {
            info!("Hello from wasm thread!");
            wasm_bindgen_futures::spawn_local(future);

            wasm_bindgen::throw_str(
                "Cursed hack to keep workers alive. See https://github.com/rustwasm/wasm-bindgen/issues/2945",
            );
        }
    });
}

pub struct UpdateHierarchyTask {
    pub wakeup_rx: async_channel::Receiver<()>,
    pub frustum_queue: Arc<ArrayQueue<Frustum>>,
    pub point_cloud: Arc<RwLock<potree::point_cloud::PotreePointCloud>>,
    pub hierarchy_snapshot_tx: async_channel::Sender<OctreeNodeSnapshot>,
}

impl UpdateHierarchyTask {
    pub async fn run(mut self) {
        let mut previous_frustum: Option<Frustum> = None;
        // Watch if we need to wakeup
        while let Ok(_) = self.wakeup_rx.recv().await {
            if let Some(frustum) = self.frustum_queue.pop() {
                if let Some(previous_frustum) = &previous_frustum {
                    if frustum_equals(previous_frustum, &frustum) {
                        // if the frustum has not changed, skip it
                        continue;
                    }
                }

                self.process_frustum(&frustum).await;

                let _ = previous_frustum.insert(frustum);
            }
        }
    }

    async fn process_frustum(&self, frustum: &Frustum) {
        let mut point_cloud = self.point_cloud.write().await;
        point_cloud
            .load_entire_hierarchy()
            .await
            .expect("Unable to load entire hierarchy");

        let hierarchy_snapshot = point_cloud.hierarchy_snapshot();
        if let Err(error) = self.hierarchy_snapshot_tx.force_send(hierarchy_snapshot) {
            warn!(
                "Failed to send hierarchy snapshot to point cloud. {:#}",
                error
            );
        }
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
