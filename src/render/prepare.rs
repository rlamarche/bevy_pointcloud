use bevy::{
    ecs::{
        query::With,
        system::{Query, Res, ResMut},
    },
    platform::collections::hash_map::Entry,
    render::{
        render_resource::{BindGroupEntries, PipelineCache, UniformBuffer},
        renderer::{RenderDevice, RenderQueue},
        sync_world::MainEntity,
    },
};

use crate::{
    PointCloud3d, PointCloudPipeline, PointCloudUniform, PreparedPointCloudUniform,
    PreparedPointCloudUniforms, RenderMaterialBindings, RenderPointCloudInstances,
    RenderPointCloudMaterialInstances,
};

/// This system prepares the point cloud uniforms.
/// Note that for the moment, all chunks of a point cloud uses the same point cloud uniform, to
/// reduce bindings upon rendering.
pub fn prepare_point_cloud_uniforms(
    point_cloud_instances: Res<RenderPointCloudInstances>,
    mesh_material_ids: Res<RenderPointCloudMaterialInstances>,
    render_material_bindings: Res<RenderMaterialBindings>,
    mut prepared_point_cloud_uniforms: ResMut<PreparedPointCloudUniforms>,
    items: Query<&MainEntity, With<PointCloud3d>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    point_cloud_pipeline: Res<PointCloudPipeline>,
    pipeline_cache: Res<PipelineCache>,
) {
    let bind_group_layout =
        pipeline_cache.get_bind_group_layout(&point_cloud_pipeline.point_cloud_uniform_layout);

    for main_entity in items {
        let Some(point_cloud_instance) = point_cloud_instances.get(main_entity) else {
            // if the instance is missing, it means that it is not visible
            // warn!("missing point cloud instance {:?}", main_entity);
            // TODO: find a faster way ? (eg: extracting only visible entities)
            continue;
        };

        let point_cloud_material = mesh_material_ids.point_cloud_material(*main_entity);
        let material_bindings_index = render_material_bindings
            .get(&point_cloud_material)
            .copied()
            .unwrap_or_default();

        let point_cloud_uniform = PointCloudUniform::new(
            &point_cloud_instance.aabb,
            point_cloud_instance.spacing.unwrap_or_default(),
            &point_cloud_instance.transforms,
            material_bindings_index.slot,
            &point_cloud_instance.splat_settings,
        );

        // create the buffer & bind group, and write it
        match prepared_point_cloud_uniforms.entry(point_cloud_instance.entity) {
            Entry::Occupied(mut entry) => {
                let value = entry.get_mut();
                value.buffer.set(point_cloud_uniform);
                value.buffer.write_buffer(&render_device, &render_queue);
            }
            Entry::Vacant(entry) => {
                let mut buffer = UniformBuffer::from(point_cloud_uniform);
                buffer.write_buffer(&render_device, &render_queue);
                let bind_group = render_device.create_bind_group(
                    "point_cloud_uniform",
                    &bind_group_layout,
                    &BindGroupEntries::single(&buffer),
                );
                entry.insert(PreparedPointCloudUniform { buffer, bind_group });
            }
        };

        // TODO cleanup unused point cloud uniforms (for removed point clouds)
    }
}
