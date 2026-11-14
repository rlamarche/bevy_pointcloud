use std::cmp::Reverse;

use bevy::{
    asset::{AssetEvent, Assets},
    camera::Camera,
    ecs::{
        message::MessageReader,
        resource::Resource,
        system::{Commands, Local, Query, Res, ResMut},
    },
    mesh::Mesh,
    platform::collections::HashMap,
};

use crate::{
    CascadesVisiblePointCloudEntities, NodeId, OctreeTopology, PointCloud, PointCloudChunk,
    PointCloudChunkKey, PointCloudInstances, PointCloudNodeKey, PointCloudServer,
    PointCloudServerSettings, PointCloudVisibilitySettings, VisiblePointCloudOctreeEntities,
};

/// Resident octree chunks, recomputed from the octrees every frame so the total cannot drift
/// when chunks are evicted or a source is replaced.
#[derive(Resource, Default)]
pub struct PointCloudTotalSize {
    pub(crate) total_size: usize,
    pub(crate) total_points: usize,
}

impl PointCloudTotalSize {
    /// Resident chunk bytes used by the server's load-admission guard.
    pub fn resident_bytes(&self) -> usize {
        self.total_size
    }

    /// Points held by resident chunks.
    pub fn resident_points(&self) -> usize {
        self.total_points
    }
}

/// The eviction frame on which each node, or one of its descendants, was last selected by a view.
#[derive(Resource, Default)]
pub struct PointCloudNodeLastSeen(HashMap<PointCloudNodeKey, u64>);

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

/// Unloads least recently seen subtrees until resident chunks fit both the byte cap
/// ([`PointCloudServerSettings::max_size`]) and twice the largest camera point budget.
///
/// Nodes selected this frame, their ancestors and roots are never evicted. A whole subtree goes
/// at once, so a resident chunk always has a resident parent.
pub fn evict_point_cloud_chunks(
    settings: Res<PointCloudServerSettings>,
    views: Query<(
        &Camera,
        &VisiblePointCloudOctreeEntities,
        &PointCloudVisibilitySettings,
    )>,
    cascades: Query<&CascadesVisiblePointCloudEntities>,
    mut point_clouds: ResMut<Assets<PointCloud>>,
    chunks: Res<Assets<PointCloudChunk>>,
    instances: Res<PointCloudInstances>,
    mut last_seen: ResMut<PointCloudNodeLastSeen>,
    mut resident: ResMut<PointCloudTotalSize>,
    mut frame: Local<u64>,
    mut commands: Commands,
) {
    *frame += 1;
    let frame = *frame;

    // Inactive cameras keep their last selection, which must not pin nodes.
    let mut point_budget = None;
    for (camera, view, view_settings) in &views {
        if camera.is_active {
            point_budget = point_budget.max(view_settings.point_budget);
            mark_seen(view, &point_clouds, &mut last_seen, frame);
        }
    }
    for cascades in &cascades {
        for (_, view) in cascades.entities.values().flatten() {
            mark_seen(view, &point_clouds, &mut last_seen, frame);
        }
    }

    let (mut bytes, mut points) = (0, 0);
    let mut candidates = Vec::new();
    for (id, cloud) in point_clouds.iter() {
        let Some(octree) = cloud.topology.as_octree() else {
            continue;
        };
        for node in octree.iter_loaded_chunks() {
            bytes += chunk_bytes(node.chunk.as_ref(), &chunks);
            points += node.point_count;
            let key = PointCloudNodeKey {
                id,
                node_id: node.id,
            };
            let seen = last_seen.0.get(&key).copied().unwrap_or(0);
            if node.depth > 0 && seen < frame {
                // Oldest first; among equally old nodes, the deepest frees the least detail.
                candidates.push((seen, Reverse(node.depth), key));
            }
        }
    }

    let point_cap = point_budget.map(|budget| budget.saturating_mul(2));
    let over = |bytes: usize, points: usize| {
        bytes > settings.max_size || point_cap.is_some_and(|cap| points > cap)
    };
    if over(bytes, points) {
        candidates.sort_unstable_by_key(|(seen, depth, _)| (*seen, *depth));
        for (_, _, key) in candidates {
            if !over(bytes, points) {
                break;
            }
            let Some(mut cloud) = point_clouds.get_mut(key.id) else {
                continue;
            };
            let Some(octree) = cloud.topology.as_octree_mut() else {
                continue;
            };
            // Already gone with an ancestor evicted earlier in this pass.
            let Some(chunk_id) = octree
                .get_node(key.node_id)
                .and_then(|node| node.chunk.as_ref())
                .map(bevy::asset::Handle::id)
            else {
                continue;
            };
            let (freed_bytes, freed_points) = evict_subtree(octree, key.node_id, &chunks);
            bytes -= freed_bytes;
            points -= freed_points;
            // `ChildrenChunks` is `linked_spawn`, so the descendants' chunk entities go too.
            if let Some(entities) = instances.get(&key.id) {
                for chunk_entities in entities.values() {
                    if let Some(&entity) = chunk_entities.get(&chunk_id) {
                        commands.entity(entity).despawn();
                    }
                }
            }
        }
    }

    // Keep stamps for resident nodes only, so the map stays as small as the cache.
    last_seen.0.retain(|key, _| {
        point_clouds
            .get(key.id)
            .and_then(|cloud| cloud.topology.as_octree())
            .and_then(|octree| octree.get_node(key.node_id))
            .is_some_and(|node| node.chunk.is_some())
    });
    resident.total_size = bytes;
    resident.total_points = points;
}

/// Stamps each selected node and its ancestors, so a parent is never older than its children.
fn mark_seen(
    view: &VisiblePointCloudOctreeEntities,
    point_clouds: &Assets<PointCloud>,
    last_seen: &mut PointCloudNodeLastSeen,
    frame: u64,
) {
    for cloud in view.entities.values() {
        let Some(octree) = point_clouds
            .get(cloud.asset_id)
            .and_then(|cloud| cloud.topology.as_octree())
        else {
            continue;
        };
        for visible in &cloud.node_entities {
            let mut next = Some(visible.id);
            while let Some(node_id) = next {
                let key = PointCloudNodeKey {
                    id: cloud.asset_id,
                    node_id,
                };
                if last_seen.0.insert(key, frame) == Some(frame) {
                    break;
                }
                next = octree.get_node(node_id).and_then(|node| node.parent_id);
            }
        }
    }
}

fn chunk_bytes(
    chunk: Option<&bevy::asset::Handle<PointCloudChunk>>,
    chunks: &Assets<PointCloudChunk>,
) -> usize {
    chunk
        .and_then(|chunk| chunks.get(chunk))
        .map_or(0, |chunk| chunk.vertex_buffer_size)
}

/// Drops the chunks of `node_id` and its resident descendants, returning the freed bytes and
/// points. Hierarchy status is kept, so visibility requests the chunks again when needed.
pub(crate) fn evict_subtree(
    octree: &mut OctreeTopology,
    node_id: NodeId,
    chunks: &Assets<PointCloudChunk>,
) -> (usize, usize) {
    let (mut bytes, mut points) = (0, 0);
    let mut stack = vec![node_id];
    while let Some(id) = stack.pop() {
        let Some(node) = octree.get_node_mut(id) else {
            continue;
        };
        let Some(chunk) = node.chunk.take() else {
            continue;
        };
        bytes += chunk_bytes(Some(&chunk), chunks);
        points += node.point_count;
        stack.extend(
            node.children_mask
                .iter_one_bits()
                .map(|i| node.children[i as usize]),
        );
    }
    (bytes, points)
}

#[cfg(test)]
mod tests {
    use bevy::{camera::primitives::Aabb, ecs::system::RunSystemOnce, prelude::*};

    use super::*;
    use crate::{InsertNodeParams, NodeData, PointCloudNodeStatus, VisiblePointCloudNodeEntity};

    const CHUNK_BYTES: usize = 100;
    const CHUNK_POINTS: usize = 10;

    struct Fixture {
        world: World,
        id: AssetId<PointCloud>,
        nodes: [NodeId; 4],
    }

    /// root ─┬─ a ── a0
    ///       └─ b
    fn fixture(max_size: usize, visible: &[usize]) -> Fixture {
        let mut world = World::new();
        let mut chunks = Assets::<PointCloudChunk>::default();
        let mut cloud = PointCloud::new_octree();
        let tree = cloud.topology.as_octree_mut().unwrap();
        let params = InsertNodeParams {
            status: PointCloudNodeStatus::Loaded,
            point_count: CHUNK_POINTS,
            aabb: Some(Aabb::from_min_max(Vec3::splat(-1.), Vec3::ONE)),
        };
        let root = tree
            .insert_root(params.clone(), NodeData::default())
            .unwrap();
        let child = |tree: &mut OctreeTopology, parent, index: u8| {
            tree.insert_child(
                parent,
                index.try_into().unwrap(),
                params.clone(),
                NodeData::default(),
            )
            .unwrap()
        };
        let a = child(tree, root, 0);
        let a0 = child(tree, a, 0);
        let b = child(tree, root, 1);
        let nodes = [root, a, a0, b];
        for node_id in nodes {
            let node = tree.get_node_mut(node_id).unwrap();
            node.chunk = Some(chunks.add(PointCloudChunk {
                topology: Default::default(),
                depth: node.depth,
                offset: None,
                mesh_handle: None,
                aabb: node.aabb,
                vertex_buffer_size: CHUNK_BYTES,
            }));
        }
        let mut clouds = Assets::<PointCloud>::default();
        let id = clouds.add(cloud).id();

        let cloud_entity = world.spawn_empty().id();
        let mut view = VisiblePointCloudOctreeEntities::default();
        let entry = view.get_mut(cloud_entity);
        entry.asset_id = id;
        entry.node_entities = visible
            .iter()
            .map(|&i| VisiblePointCloudNodeEntity {
                id: nodes[i],
                ..default()
            })
            .collect();
        world.spawn((
            Camera::default(),
            view,
            PointCloudVisibilitySettings {
                point_budget: None,
                ..default()
            },
        ));

        world.insert_resource(clouds);
        world.insert_resource(chunks);
        world.insert_resource(PointCloudServerSettings {
            max_size,
            ..default()
        });
        world.init_resource::<PointCloudInstances>();
        world.init_resource::<PointCloudNodeLastSeen>();
        world.init_resource::<PointCloudTotalSize>();
        Fixture { world, id, nodes }
    }

    fn resident(fixture: &Fixture) -> Vec<bool> {
        let clouds = fixture.world.resource::<Assets<PointCloud>>();
        let tree = clouds
            .get(fixture.id)
            .unwrap()
            .topology
            .as_octree()
            .unwrap();
        fixture
            .nodes
            .iter()
            .map(|&id| tree.get_node(id).unwrap().chunk.is_some())
            .collect()
    }

    #[test]
    fn evicts_unseen_subtrees_until_under_the_cap() {
        // Four 100-byte chunks against a 250-byte cap; only `b` is selected.
        let mut fixture = fixture(250, &[3]);
        fixture
            .world
            .run_system_once(evict_point_cloud_chunks)
            .unwrap();

        // `a0` then `a` go; `b` is selected and the root is never evicted.
        assert_eq!(resident(&fixture), [true, false, false, true]);
        let total = fixture.world.resource::<PointCloudTotalSize>();
        assert_eq!(total.resident_bytes(), 2 * CHUNK_BYTES);
        assert_eq!(total.resident_points(), 2 * CHUNK_POINTS);
    }

    #[test]
    fn selected_nodes_pin_their_ancestors() {
        // Selecting `a0` pins `a` too, leaving only `b` to evict, even though
        // the cache stays over its cap.
        let mut fixture = fixture(0, &[2]);
        fixture
            .world
            .run_system_once(evict_point_cloud_chunks)
            .unwrap();

        assert_eq!(resident(&fixture), [true, true, true, false]);
        let total = fixture.world.resource::<PointCloudTotalSize>();
        assert_eq!(total.resident_bytes(), 3 * CHUNK_BYTES);
    }

    #[test]
    fn evicting_a_node_unloads_its_whole_subtree() {
        let mut fixture = fixture(usize::MAX, &[]);
        let chunks = Assets::<PointCloudChunk>::default();
        let [_, a, ..] = fixture.nodes;
        {
            let mut clouds = fixture.world.resource_mut::<Assets<PointCloud>>();
            let mut cloud = clouds.get_mut(fixture.id).unwrap();
            let tree = cloud.topology.as_octree_mut().unwrap();
            // Unknown chunk assets count as zero bytes but their points still count.
            assert_eq!(evict_subtree(tree, a, &chunks), (0, 2 * CHUNK_POINTS));
        }
        assert_eq!(resident(&fixture), [true, false, false, true]);
    }
}
