use crate::{
    bevy::prelude::*,
    octree::{
        asset::Octree,
        extract::{
            resources::{ExtractOctreeNodeEvictionQueue, NodeAllocation, OctreeNodeAllocations},
            OctreeNodeExtraction,
        },
        node::NodeData,
        visibility::resources::GlobalVisibleOctreeNodes,
    },
};

/// This system allocates gpu memory for computed visible octree nodes, and trace allocations for later extraction.
/// It frees gpu memory of nodes that aren't visible, if needed.
#[cfg_attr(feature = "trace", tracing::instrument(skip_all))]
pub fn allocate_visible_octree_nodes<E: OctreeNodeExtraction>(
    global_visible_octree_nodes: Res<GlobalVisibleOctreeNodes<E::NodeData>>,
    octrees: Res<Assets<Octree<E::NodeData>>>,
    mut octree_buffer_allocator: ResMut<OctreeNodeAllocations<E>>,
    mut extract_octree_node_eviction_queue: ResMut<ExtractOctreeNodeEvictionQueue<E>>,
) {
    // clear previously allocated nodes
    octree_buffer_allocator.allocated_nodes_this_frame.clear();
    octree_buffer_allocator.freed_nodes_this_frame.clear();

    // iterate through nodes that needs allocations
    // note that they are ordered by priority because in insertion order
    for (octree_node_key, &weight) in &global_visible_octree_nodes.visible_octree_nodes {
        // skip already allocated nodes
        // TODO find a way to not iterate all visible nodes
        if octree_buffer_allocator
            .allocations
            .contains_key(octree_node_key)
        {
            continue;
        }

        let Some(octree) = octrees.get(octree_node_key.octree_id) else {
            debug!("Visible octree not found: {}", octree_node_key.octree_id);
            continue;
        };

        let Some(node) = octree.node(octree_node_key.node_id) else {
            debug!(
                "Visible node not found: {}/{}",
                octree_node_key.octree_id, octree_node_key.node_id
            );
            continue;
        };

        let Some(node_data) = &node.data else {
            // Node data not available, skip it
            continue;
        };

        let instance_count = node_data.instance_count();

        // try to allocate memory for this node
        let mut allocation = None;
        while allocation.is_none() {
            allocation = match octree_buffer_allocator
                .allocator
                .allocate(instance_count as u32)
            {
                Some(allocation) => Some(allocation),
                None => {
                    // allocation have failed, we have to free space

                    // try to pop an evictable node
                    if let Some((evictable_node_key, _)) = extract_octree_node_eviction_queue
                        .eviction_queue
                        .pop_if(|key, eviction_priority| {
                            !global_visible_octree_nodes
                                .visible_octree_nodes
                                .contains_key(key)
                                || eviction_priority.0.weight < weight
                        })
                    {
                        if let Some(evictable_allocation) = octree_buffer_allocator
                            .allocations
                            .remove(&evictable_node_key)
                        {
                            if let Some(octree) = octrees.get(evictable_node_key.octree_id) {
                                if let Some(node) = octree.node(evictable_node_key.node_id) {
                                    debug!("Free node {}", node.hierarchy.name);
                                }
                            }

                            octree_buffer_allocator
                                .allocator
                                .free(evictable_allocation.allocation);
                            octree_buffer_allocator
                                .freed_nodes_this_frame
                                .push(evictable_allocation);
                        } else {
                            warn!(
                                "Allocation for node {:?} not found, should'nt happen.",
                                evictable_node_key
                            );
                        }
                        // try again to allocate on next iteration
                        continue;
                    } else {
                        // there is nothing to unallocate, exit the loop
                        break;
                    }
                }
            }
        }

        if let Some(allocation) = allocation {
            debug!("Allocated node {}", node.hierarchy.name);
            let start = allocation.offset;

            let node_allocation = NodeAllocation {
                octree_node_key: octree_node_key.clone(),
                allocation,
                start,
                count: instance_count as u32,
            };

            // store the allocation infos
            octree_buffer_allocator
                .allocations
                .insert(octree_node_key.clone(), node_allocation.clone());

            octree_buffer_allocator
                .allocated_nodes_this_frame
                .push(node_allocation);
        } else {
            // info!("GPU memory is full");
            // do not try to load another node in gpu memory
            break;
        }
    }
}
