use super::asset::RenderOctreeNodeAllocation;
use super::asset::RenderOctreeNodeData;
use super::components::RenderVisibleOctreeNodes;
use super::node::RenderOctreeNode;
use super::resources::{ExtractedOctreeNodes, RenderOctreeIndex};
use crate::octree::asset::Octree;
use crate::octree::extract::OctreeNodeExtraction;
use crate::octree::extract::resources::OctreeNodeAllocations;
use crate::octree::visibility::components::ViewVisibleOctreeNodes;
use bevy_asset::Assets;
use bevy_camera::Camera;
use bevy_ecs::prelude::*;
use bevy_log::prelude::*;
use bevy_render::Extract;
use bevy_render::sync_world::RenderEntity;

use std::marker::PhantomData;

/// This system extracts computed visible octree nodes and add them in the render world, for each view (camera)
#[cfg_attr(feature = "trace", tracing::instrument(skip_all))]
pub fn extract_visible_octree_nodes<E: OctreeNodeExtraction, A: RenderOctreeNode>(
    mut commands: Commands,
    query: Extract<
        Query<
            (
                RenderEntity,
                &ViewVisibleOctreeNodes<E::NodeData, E::Component>,
            ),
            With<Camera>,
        >,
    >,
    mapper: Extract<Query<&RenderEntity>>,
    mut render_octree_index: ResMut<RenderOctreeIndex<E::Component>>,
) where
    A: RenderOctreeNode<SourceOctreeNode = E::NodeData>,
{
    for (render_entity, visible_point_cloud_octree_3d_nodes) in query.iter() {
        let render_visible_point_cloud_octree_3d_nodes =
            RenderVisibleOctreeNodes::<E::NodeData, E::Component> {
                octrees: visible_point_cloud_octree_3d_nodes
                    .octrees
                    .clone()
                    .into_iter()
                    // for each visible octree, extract visible nodes, and store them using the render entity reference
                    .filter_map(|(entity, data)| {
                        let Ok(render_entity) = mapper.get(entity) else {
                            warn!("Render entity for PointCloudOctree3d not found");
                            return None;
                        };

                        // makes sure an index exists for this entity
                        render_octree_index.add_octree(render_entity.id());

                        Some((render_entity.id(), data))
                    })
                    .collect(),
                _phantom_data: PhantomData,
            };
        commands
            .entity(render_entity)
            .insert(render_visible_point_cloud_octree_3d_nodes);
    }
}

/// This system extract data for octree nodes pre-allocated, and mark for removal octree nodes pre-freed
pub fn extract_octree_node_allocations<E: OctreeNodeExtraction>(
    allocations: Extract<Res<OctreeNodeAllocations<E>>>,
    octrees: Extract<Res<Assets<Octree<E::NodeData>>>>,
    // mut render_allocations: ResMut<RenderOctreeNodeAllocations<E>>,
    // mut render_octrees: ResMut<RenderOctrees<A>>,
    mut extracted_octree_nodes: ResMut<ExtractedOctreeNodes<E>>,
) {
    if extracted_octree_nodes.max_instances == 0 {
        // TODO handle if value changed
        extracted_octree_nodes.max_instances = allocations.max_instances;
    }
    // clear tracking
    // clear previously computed data
    extracted_octree_nodes.clear_all();
    extracted_octree_nodes
        .allocated_nodes_this_frame
        .extend_from_slice(&allocations.allocated_nodes_this_frame);
    extracted_octree_nodes
        .freed_nodes_this_frame
        .extend_from_slice(&allocations.freed_nodes_this_frame);

    // track freed nodes
    for freed_node in &allocations.freed_nodes_this_frame {
        extracted_octree_nodes
            .removed_nodes
            .entry(freed_node.octree_node_key.octree_id)
            .or_default()
            .push(freed_node.octree_node_key.node_id);
    }

    // TODO group allocations by octree to save some lookups
    for allocated_node in &allocations.allocated_nodes_this_frame {
        // track added nodes
        extracted_octree_nodes
            .added_nodes
            .entry(allocated_node.octree_node_key.octree_id)
            .or_default()
            .push(allocated_node.octree_node_key.node_id);

        // load the octree
        let Some(octree) = octrees.get(allocated_node.octree_node_key.octree_id) else {
            debug!(
                "Octree asset {:?} not found when extracting octree nodes",
                allocated_node.octree_node_key.octree_id
            );
            continue;
        };

        // load the node
        let Some(octree_node) = octree.node(allocated_node.octree_node_key.node_id) else {
            debug!(
                "Octree node {:?} not found in asset {:?}",
                allocated_node.octree_node_key, allocated_node.octree_node_key.octree_id
            );
            continue;
        };

        // get the render octree hashmap
        let render_octree =
            extracted_octree_nodes.get_or_create_mut(allocated_node.octree_node_key.octree_id);

        // extract octree node data
        if let Some(data) = E::extract_octree_node(octree_node) {
            // store extracted data in render octree
            render_octree.insert(
                allocated_node.octree_node_key.node_id,
                RenderOctreeNodeData::<E::ExtractedNodeData> {
                    id: octree_node.hierarchy.id,
                    parent_id: octree_node.hierarchy.parent_id,
                    child_index: octree_node.hierarchy.child_index,
                    children: octree_node.hierarchy.children.clone(),
                    children_mask: octree_node.hierarchy.children_mask.clone(),
                    depth: octree_node.hierarchy.depth,
                    bounding_box: octree_node.hierarchy.bounding_box.clone(),
                    data,
                    allocation: RenderOctreeNodeAllocation {
                        start: allocated_node.start,
                        end: allocated_node.end,
                    },
                },
            );
        }
    }
}
