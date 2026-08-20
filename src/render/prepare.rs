use std::ops::ControlFlow;

use bevy::{
    core_pipeline::core_3d::Opaque3d,
    ecs::{
        entity::Entity,
        query::With,
        system::{Commands, Local, Query, Res, ResMut},
    },
    log::{info, warn},
    pbr::{LightEntity, Shadow},
    platform::collections::{hash_map::Entry, HashMap},
    render::{
        camera::ExtractedCamera,
        render_phase::ViewBinnedRenderPhases,
        render_resource::{
            BindGroupEntries, Extent3d, PipelineCache, TexelCopyBufferLayout, TextureDescriptor,
            TextureDimension, TextureFormat::Rgba8Uint, TextureUsages, UniformBuffer,
        },
        renderer::{RenderDevice, RenderQueue},
        sync_world::MainEntity,
        texture::{ColorAttachment, TextureCache},
        view::{ExtractedView, Msaa},
    },
};
use bytemuck::{Pod, Zeroable};
use wgpu::Color as WgpuColor;

use crate::{
    NodeId, PointCloud3d, PointCloudPipeline, PointCloudUniform, PreparedPointCloudUniform,
    PreparedPointCloudUniforms, RenderMaterialBindings, RenderOctreeInstancesIndex,
    RenderPointCloudInstances, RenderPointCloudMaterialInstances,
    RenderShadowMapVisiblePointCloudEntities, RenderVisiblePointCloudEntities, VisibleNodesTexture,
    VisibleNodesTextureBindGroup,
};

pub const MAX_NODES: usize = 2048;

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
    render_octree_instances_index: Res<RenderOctreeInstancesIndex>,
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
            &point_cloud_instance.model_aabb,
            render_octree_instances_index
                .index
                .get(&point_cloud_instance.render_entity)
                .map(|index| index.index())
                .unwrap_or(u32::MAX),
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

/// The data layout for the texture containing visible nodes data
#[derive(Default, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct VisibleOctreeNodeUniform {
    // the children mask
    pub children_mask: u8,
    pub offset: u8,
    // index of the first child
    pub first_child_index: u16,
}

impl ::core::fmt::Debug for VisibleOctreeNodeUniform {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        write!(
            f,
            "mask = {:b} first_child_index = {:?}",
            self.children_mask, self.first_child_index
        )?;
        Ok(())
    }
}

pub fn prepare_camera_visible_nodes_texture(
    mut commands: Commands,
    mut texture_cache: ResMut<TextureCache>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    render_octree_index: Res<RenderOctreeInstancesIndex>,
    opaque_phases: Res<ViewBinnedRenderPhases<Opaque3d>>,
    views_3d: Query<
        (Entity, &ExtractedView, &RenderVisiblePointCloudEntities),
        With<ExtractedCamera>,
    >,
    mut visible_nodes_buffer: Local<Vec<VisibleOctreeNodeUniform>>,
) {
    // for each view
    for (entity, extracted_view, visible_nodes) in &views_3d {
        // skip if no phases
        // TODO: add other phases types here ?
        if !opaque_phases.contains_key(&extracted_view.retained_view_entity) {
            continue;
        };

        prepare_visible_nodes_texture(
            &mut commands,
            &mut texture_cache,
            &render_device,
            &render_queue,
            &render_octree_index,
            &mut visible_nodes_buffer,
            entity,
            visible_nodes,
        );
    }
}

pub fn prepare_cascades_visible_nodes_texture(
    mut commands: Commands,
    mut texture_cache: ResMut<TextureCache>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    render_octree_index: Res<RenderOctreeInstancesIndex>,
    shadow_phases: Res<ViewBinnedRenderPhases<Shadow>>,
    views_3d: Query<(Entity, &ExtractedView, &LightEntity)>,
    mut visible_nodes_buffer: Local<Vec<VisibleOctreeNodeUniform>>,
    shadow_map_visible_entities: Query<&RenderShadowMapVisiblePointCloudEntities>,
) {
    // for each view
    for (entity, extracted_view, light_entity) in &views_3d {
        // skip if no phases
        if !shadow_phases.contains_key(&extracted_view.retained_view_entity) {
            continue;
        };

        let LightEntity::Directional {
            light_entity,
            cascade_index: _,
        } = light_entity
        else {
            continue;
        };

        let Ok(shadow_map_visible_entities) = shadow_map_visible_entities.get(*light_entity) else {
            continue;
        };

        let Some(visible_nodes) = shadow_map_visible_entities
            .subviews
            .get(&extracted_view.retained_view_entity)
        else {
            warn!(
                "Shadow map visible entities not foud for {:?}",
                extracted_view.retained_view_entity
            );
            continue;
        };

        prepare_visible_nodes_texture(
            &mut commands,
            &mut texture_cache,
            &render_device,
            &render_queue,
            &render_octree_index,
            &mut visible_nodes_buffer,
            entity,
            visible_nodes,
        );
    }
}

pub fn prepare_visible_nodes_texture_bind_group(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    point_cloud_pipeline: Res<PointCloudPipeline>,
    render_device: Res<RenderDevice>,
    views: Query<(Entity, &VisibleNodesTexture)>,
) {
    let layout = &pipeline_cache
        .get_bind_group_layout(&point_cloud_pipeline.point_cloud_octree_visible_nodes_layout);
    for (entity, prepass_textures) in &views {
        let Some(texture) = &prepass_textures.visible_nodes else {
            warn!("No visible nodes pass texture for {}", entity);
            continue;
        };

        let texture_view = texture.texture.default_view.clone();

        commands
            .entity(entity)
            .insert(VisibleNodesTextureBindGroup {
                texture: render_device.create_bind_group(
                    "pointcloud_octree_visible_nodes",
                    layout,
                    &BindGroupEntries::single(&texture_view),
                ),
            });
    }
}

/// Prepare a visible nodes texture for given visible nodes `visible_nodes`
fn prepare_visible_nodes_texture(
    commands: &mut Commands<'_, '_>,
    texture_cache: &mut TextureCache,
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
    render_octree_index: &RenderOctreeInstancesIndex,
    visible_nodes_buffer: &mut Vec<VisibleOctreeNodeUniform>,
    entity: Entity,
    visible_nodes: &RenderVisiblePointCloudEntities,
) {
    let octrees_count = visible_nodes
        .entities
        .values()
        .filter(|visible_entity| {
            render_octree_index
                .index
                .contains_key(&visible_entity.entity)
        })
        .count();
    if octrees_count == 0 {
        return;
    }
    let required_buffer_size = octrees_count * MAX_NODES;

    // reuse allocations
    if visible_nodes_buffer.len() < required_buffer_size {
        visible_nodes_buffer.resize(required_buffer_size, VisibleOctreeNodeUniform::default());
    }

    // get the texture for containing visible nodes data
    let visible_nodes_texture = {
        // The size of the depth texture
        let size = Extent3d {
            width: MAX_NODES as u32,
            height: octrees_count as u32,
            depth_or_array_layers: 1,
        };

        let descriptor = TextureDescriptor {
            label: Some("pcl_visible_nodes_texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: Rgba8Uint,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        };

        texture_cache.get(render_device, descriptor)
    };
    let mut node_index = vec![HashMap::<NodeId, u32>::default(); render_octree_index.slab.len()];
    for (_main_entity, visible_point_cloud) in &visible_nodes.entities {
        let octree_index = render_octree_index
            .get(visible_point_cloud.entity)
            .expect("octree index out of bounds")
            .index() as usize;

        let node_mapping = &mut node_index[octree_index];
        let base_offset = octree_index * MAX_NODES;

        for (i, visible_node) in visible_point_cloud.chunk_entities.iter().enumerate() {
            if i >= MAX_NODES {
                // warn!("Too many nodes in octree, some will be ignored.");
                break;
            }

            let offset = visible_node.offset;

            visible_nodes_buffer[base_offset + i] = VisibleOctreeNodeUniform {
                children_mask: visible_node.children_mask.bits(),
                offset,
                first_child_index: visible_node.first_child_index as u16,
            };

            node_mapping.insert(visible_node.id, i as u32);
        }
    }
    render_queue.write_texture(
        visible_nodes_texture.texture.as_image_copy(),
        bytemuck::cast_slice(&*visible_nodes_buffer),
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some((MAX_NODES * 4) as u32), // 4 bytes par texel RGBA8Uint
            rows_per_image: Some(octrees_count as u32),
        },
        Extent3d {
            width: MAX_NODES as u32,
            height: octrees_count as u32,
            depth_or_array_layers: 1,
        },
    );
    commands.entity(entity).insert(VisibleNodesTexture {
        visible_nodes: Some(ColorAttachment::new(
            visible_nodes_texture,
            None,
            None,
            Some(WgpuColor::TRANSPARENT),
        )),
        node_index,
    });
}
