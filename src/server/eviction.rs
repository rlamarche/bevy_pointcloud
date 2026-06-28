use std::cmp::Reverse;

use bevy::{
    asset::AssetEvent, ecs::{
        message::MessageReader,
        resource::Resource,
        system::{Res, ResMut},
    }, mesh::Mesh, time::{Real, Time},
};
use priority_queue::PriorityQueue;

use crate::{
    GlobalVisiblePointCloudNodes, PointCloud, PointCloudChunkKey, PointCloudNodeKey,
    PointCloudServer,
};

#[derive(Resource, Default)]
pub struct PointCloudTotalSize {
    pub(crate) total_size: usize,
}

/// This resource contains a priority queue to determine which nodes to evict first.
/// Nodes that are seen less recently are first in this queue.
#[derive(Resource, Default)]
pub struct PointCloudServerEvictionQueue {
    pub(crate) eviction_queue: PriorityQueue<PointCloudNodeKey, Reverse<u128>>,
}

#[derive(Resource, Default)]
pub struct PointCloudTracking {
    pub(crate) loaded_chunks: Vec<(PointCloudChunkKey, Mesh)>,
}

/// Remove loader from cache upon point cloud asset removal
pub fn cleanup_loaders(
    server: Res<PointCloudServer>,
    mut events: MessageReader<AssetEvent<PointCloud>>,
) {
    let mut loaders = server.write_loaders();
    for event in events.read() {
        if let AssetEvent::Removed { id } = event {
            loaders.remove(id);
        }
    }
}

/// This system update the point cloud node eviction queue with latest informations
pub fn update_point_cloud_server_node_eviction_queue(
    mut point_cloud_node_eviction_queue: ResMut<PointCloudServerEvictionQueue>,
    global_visible_point_cloud_nodes: Res<GlobalVisiblePointCloudNodes>,
    time: Res<Time<Real>>,
) {
    let elapsed = time.elapsed().as_millis();
    let eviction_queue = &mut point_cloud_node_eviction_queue.eviction_queue;

    for (key, _) in &global_visible_point_cloud_nodes.visible_nodes {
        eviction_queue.push(key.clone(), Reverse(elapsed));
    }
}

// /// This system remove nodes data to meet memory budget requirements, and update octree node
// /// TODO implement it
// pub fn evict_point_cloud_chunks(
//     settings: Res<PointCloudServerSettings>,
//     mut octree_node_eviction_queue: ResMut<PointCloudServerEvictionQueue>,
//     global_visible_octree_nodes: Res<GlobalVisiblePointCloudNodes>,
//     mut point_cloud_total_size: ResMut<PointCloudTotalSize>,
//     mut octrees: ResMut<Assets<PointCloud>>,
//     mut chunks: ResMut<Assets<PointCloudChunk>>,
// ) {
//     let eviction_queue = &mut octree_node_eviction_queue.eviction_queue;

//     let total_size = &mut point_cloud_total_size.total_size;

//     while *total_size > settings.max_size {
//         if let Some((key, _)) = eviction_queue
//             .pop_if(|key, _| !global_visible_octree_nodes.visible_nodes.contains_key(key))
//         {
//             let Some(mut point_cloud) = octrees.get_mut(key.id) else {
//                 continue;
//             };

//             if let Some(hierarchy_node) = point_cloud.hierarchy.get_node_mut(key.node_id)
//                 && let Some(chunk_handle) = &hierarchy_node.chunk
//                 && let Some(point_cloud_chunk) = chunks.get_mut(chunk_handle.id())
//             {
//                 *total_size -= point_cloud_chunk.vertex_buffer_size;
//             }
//         } else {
//             break;
//         }
//     }
// }
