use crate::{
    bevy::prelude::*,
    octree::extract::render::{components::RenderVisibleOctreeNodes, resources::RenderOctrees},
    pointcloud_octree::{
        asset::data::PointCloudNodeData, component::PointCloudOctree3d,
        extract::RenderPointCloudNodeData, render::prepare::MAX_NODES,
    },
    render::mesh::PointCloudMesh,
};
use bevy_camera::Camera3d;
use bevy_platform::collections::{HashMap, HashSet};
use bevy_render::{
    batching::gpu_preprocessing::IndirectParametersNonIndexed,
    render_resource::{BufferUsages, BufferVec},
    renderer::{RenderDevice, RenderQueue},
};

/// Stores multi draw indirect buffer for each view
#[derive(Default, Component)]
pub struct RenderVisibleNodesIndirectBuffers {
    buffers: HashMap<Entity, BufferVec<IndirectParametersNonIndexed>>,
}

impl RenderVisibleNodesIndirectBuffers {
    fn get_or_insert_mut(
        &mut self,
        entity: Entity,
        device: &RenderDevice,
    ) -> &mut BufferVec<IndirectParametersNonIndexed> {
        self.buffers.entry(entity).or_insert_with(|| {
            let mut buffer = BufferVec::new(BufferUsages::STORAGE | BufferUsages::INDIRECT);
            buffer.reserve(MAX_NODES, device);
            buffer
        })
    }

    pub fn get(
        &self,
        entity: &Entity,
    ) -> Option<&BufferVec<IndirectParametersNonIndexed>> {
        self.buffers.get(entity)
    }

    fn remove(
        &mut self,
        entity: &Entity,
    ) -> Option<BufferVec<IndirectParametersNonIndexed>> {
        self.buffers.remove(entity)
    }
}

pub fn prepare_indirect_buffer(
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    views_3d: Query<
        (
            &RenderVisibleOctreeNodes<PointCloudNodeData, PointCloudOctree3d>,
            &mut RenderVisibleNodesIndirectBuffers,
        ),
        With<Camera3d>,
    >,
    render_octrees: Res<RenderOctrees<RenderPointCloudNodeData>>,
    point_cloud_mesh: Res<PointCloudMesh>,
    mut removed_entities: Local<HashSet<Entity>>,
) {
    // for each camera
    for (visible_octree_nodes, mut render_visible_nodes_indirect_buffers) in views_3d {
        if !visible_octree_nodes.changed_this_frame {
            continue;
        }

        // clear removed entities tracking
        removed_entities.clear();

        // insert all current entities
        for entity in render_visible_nodes_indirect_buffers.buffers.keys() {
            removed_entities.insert(*entity);
        }

        for (entity, (asset_id, visible_nodes)) in &visible_octree_nodes.octrees {
            let Some(render_octree) = render_octrees.get(*asset_id) else {
                warn!("Missing octree when preparing indirect buffer");
                continue;
            };

            // remove this entity from the `removed_entities` hashset
            removed_entities.remove(entity);

            // get this entity's indirect render buffer
            let indirect_buffer =
                render_visible_nodes_indirect_buffers.get_or_insert_mut(*entity, &render_device);

            // clear previous data
            indirect_buffer.clear();

            for visible_node in visible_nodes {
                // lookup node informations
                // TODO put node allocation informatins in [`VisibleOctreeNode`] to prevent lookup ?
                let Some(node) = render_octree.nodes.get(&visible_node.id) else {
                    continue;
                };

                indirect_buffer.push(IndirectParametersNonIndexed {
                    vertex_count: point_cloud_mesh.index_count,
                    instance_count: node.allocation.count,
                    base_vertex: 0,
                    first_instance: node.allocation.start,
                });
            }

            indirect_buffer.write_buffer(&render_device, &render_queue);
        }

        // cleanup removed entities
        for removed_entity in &removed_entities {
            render_visible_nodes_indirect_buffers.remove(removed_entity);
        }
    }
}
