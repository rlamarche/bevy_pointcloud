use bevy::{
    ecs::{
        entity::Entity,
        system::{Commands, Query, Res},
    },
    render::{
        mesh::allocator::MeshAllocator,
        render_resource::{BindGroupEntries, PipelineCache, UniformBuffer},
        renderer::{RenderDevice, RenderQueue},
        sync_world::MainEntity,
    },
};

use crate::{
    PointCloudPipeline, PointCloudUniform, PreparedPointCloudUniform, RenderMaterialBindings,
    RenderMaterialInstances, RenderPointCloudInstances,
};

/// Creates batches for a render phase that uses bins, when GPU batch data
/// building isn't in use.
pub fn prepare_point_cloud_uniforms(
    point_cloud_instances: Res<RenderPointCloudInstances>,
    mesh_material_ids: Res<RenderMaterialInstances>,
    render_material_bindings: Res<RenderMaterialBindings>,
    mesh_allocator: Res<MeshAllocator>,
    items: Query<(Entity, &MainEntity)>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    point_cloud_pipeline: Res<PointCloudPipeline>,
    pipeline_cache: Res<PipelineCache>,
    mut commands: Commands,
) {
    let mut batch_insert = Vec::new();

    for (render_entity, main_entity) in items {
        let Some(point_cloud_instance) = point_cloud_instances.get(main_entity) else {
            continue;
        };

        let mesh_material = mesh_material_ids.mesh_material(*main_entity);
        let material_bindings_index = render_material_bindings
            .get(&mesh_material)
            .copied()
            .unwrap_or_default();

        let first_vertex_index =
            match mesh_allocator.mesh_vertex_slice(&point_cloud_instance.mesh_id) {
                Some(mesh_vertex_slice) => mesh_vertex_slice.range.start,
                None => 0,
            };

        let point_cloud_uniform = PointCloudUniform::new(
            &point_cloud_instance.aabb,
            &point_cloud_instance.transforms,
            first_vertex_index,
            material_bindings_index.slot,
        );

        let mut buffer = UniformBuffer::from(point_cloud_uniform);
        buffer.write_buffer(&render_device, &render_queue);

        let bind_group_layout =
            pipeline_cache.get_bind_group_layout(&point_cloud_pipeline.point_cloud_uniform_layout);

        let bind_group = render_device.create_bind_group(
            "point_cloud_uniform",
            &bind_group_layout,
            &BindGroupEntries::single(&buffer),
        );

        batch_insert.push((render_entity, PreparedPointCloudUniform { bind_group }));

        // commands
        //     .entity(render_entity)
        //     .insert(PreparedPointCloudUniform { bind_group });
    }

    commands.insert_batch(batch_insert);
}
