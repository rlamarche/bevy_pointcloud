use bevy::{
    core_pipeline::core_3d::Opaque3d,
    ecs::{
        entity::Entity,
        query::With,
        system::{Commands, Local, Query, Res, ResMut},
    },
    log::{info, warn},
    pbr::{LightEntity, MeshUniform, RenderMeshInstances, Shadow},
    platform::collections::{hash_map::Entry, HashMap},
    render::{
        camera::ExtractedCamera,
        mesh::{allocator::MeshAllocator, RenderMesh},
        render_asset::RenderAssets,
        render_phase::ViewBinnedRenderPhases,
        render_resource::{
            BindGroupEntries, Extent3d, GpuArrayBuffer, PipelineCache, TexelCopyBufferLayout,
            TextureDescriptor, TextureDimension, TextureFormat::Rgba8Uint, TextureUsages,
            UniformBuffer,
        },
        renderer::{RenderDevice, RenderQueue},
        sync_world::MainEntity,
        texture::TextureCache,
        view::ExtractedView,
    },
};
use bytemuck::{Pod, Zeroable};

use crate::{
    FallbackVisibleNodesTexture, NodeId, PointCloud3d, PointCloudPipeline, PointCloudUniform,
    PreparedPointCloudUniform, PreparedPointCloudUniforms, RenderMaterialBindings,
    RenderOctreeInstancesIndex, RenderPointCloudChunk, RenderPointCloudInstances,
    RenderPointCloudMaterialInstances, RenderShadowMapVisiblePointCloudEntities,
    RenderVisiblePointCloudEntities, ViewPointCloudBindGroups, VisibleNodesTexture,
};

pub const MAX_NODES: usize = 2048;

/// This system prepares the point cloud uniforms.
/// Note that for the moment, all chunks of a point cloud uses the same point cloud uniform, to
/// reduce bindings upon rendering.
pub fn prepare_point_cloud_uniforms(
    point_cloud_instances: Res<RenderPointCloudInstances>,
    mesh_instances: Res<RenderMeshInstances>,
    mesh_allocator: Res<MeshAllocator>,
    mesh_material_ids: Res<RenderPointCloudMaterialInstances>,
    render_material_bindings: Res<RenderMaterialBindings>,
    mut prepared_point_cloud_uniforms: ResMut<PreparedPointCloudUniforms>,
    items: Query<&MainEntity, With<PointCloud3d>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    render_octree_instances_index: Res<RenderOctreeInstancesIndex>,
) {
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

        let Some(mesh_asset_id) = mesh_instances.mesh_asset_id(*main_entity) else {
            warn!("Mesh asset id missing for point cloud {:?}", main_entity);
            continue;
        };
        let first_vertex_index = match mesh_allocator.mesh_vertex_slice(&mesh_asset_id) {
            Some(mesh_vertex_slice) => mesh_vertex_slice.range.start,
            None => 0,
        };

        let mesh_uniform = MeshUniform::new(
            &point_cloud_instance.transforms,
            first_vertex_index,
            material_bindings_index.slot,
            None,
            None,
            None,
            None,
        );

        let point_cloud_uniform = PointCloudUniform::new(
            &point_cloud_instance.aabb,
            &point_cloud_instance.model_aabb,
            render_octree_instances_index
                .index
                .get(&point_cloud_instance.render_entity)
                .map(super::resources::OctreeInstanceIndex::index)
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
                value.mesh_uniform_buffer.clear();
                value.mesh_uniform_buffer.push(mesh_uniform);
                value
                    .mesh_uniform_buffer
                    .write_buffer(&render_device, &render_queue);
                value.point_cloud_uniform_buffer.set(point_cloud_uniform);
                value
                    .point_cloud_uniform_buffer
                    .write_buffer(&render_device, &render_queue);
            }
            Entry::Vacant(entry) => {
                let mut mesh_uniform_buffer = GpuArrayBuffer::new(&render_device.limits());
                mesh_uniform_buffer.push(mesh_uniform);
                mesh_uniform_buffer.write_buffer(&render_device, &render_queue);
                let mut point_cloud_uniform_buffer = UniformBuffer::from(point_cloud_uniform);
                point_cloud_uniform_buffer.write_buffer(&render_device, &render_queue);

                entry.insert(PreparedPointCloudUniform {
                    mesh_uniform_buffer,
                    point_cloud_uniform_buffer,
                });
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
    render_point_cloud_chunks: Res<RenderAssets<RenderPointCloudChunk>>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    mesh_allocator: Res<MeshAllocator>,
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
            &render_point_cloud_chunks,
            &render_meshes,
            &mesh_allocator,
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
    render_point_cloud_chunks: Res<RenderAssets<RenderPointCloudChunk>>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    mesh_allocator: Res<MeshAllocator>,
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
                "Shadow map visible entities not found for {:?}",
                extracted_view.retained_view_entity
            );
            continue;
        };

        prepare_visible_nodes_texture(
            &mut commands,
            &render_point_cloud_chunks,
            &render_meshes,
            &mesh_allocator,
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

pub fn prepare_visible_nodes_texture_bind_groups(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    point_cloud_pipeline: Res<PointCloudPipeline>,
    render_device: Res<RenderDevice>,
    views: Query<(Entity, Option<&VisibleNodesTexture>)>,
    mut existing_view_point_cloud_bind_groups: Query<&mut ViewPointCloudBindGroups>,
    prepared_point_cloud_uniforms: Res<PreparedPointCloudUniforms>,
    items: Query<&MainEntity, With<PointCloud3d>>,
    fallback_visible_nodes_texture: Res<FallbackVisibleNodesTexture>,
) {
    let layout = &pipeline_cache.get_bind_group_layout(&point_cloud_pipeline.point_cloud_layout);
    for (entity, visible_nodes_texture) in &views {
        let texture_view = match visible_nodes_texture {
            Some(visible_nodes_texture) => visible_nodes_texture.texture.default_view.clone(),
            None => fallback_visible_nodes_texture.texture_view.clone(),
        };

        // Fetch or create the `ViewPointCloudBindGroups` component
        let mut view_point_cloud_bind_groups =
            match existing_view_point_cloud_bind_groups.get_mut(entity) {
                Ok(ref mut view_point_cloud_bind_groups) => {
                    std::mem::take(&mut **view_point_cloud_bind_groups)
                }
                Err(_) => ViewPointCloudBindGroups::default(),
            };

        for main_entity in items {
            let Some(prepared_uniform) = prepared_point_cloud_uniforms.get(main_entity) else {
                continue;
            };

            // TODO reuse previous bind group ?
            let bind_group = render_device.create_bind_group(
                "view_point_cloud",
                layout,
                &BindGroupEntries::sequential((
                    prepared_uniform.mesh_uniform_buffer.binding().unwrap(),
                    &prepared_uniform.point_cloud_uniform_buffer,
                    &texture_view,
                )),
            );

            view_point_cloud_bind_groups
                .bind_groups
                .insert(*main_entity, bind_group);
        }

        let mut entity_commands = commands.entity(entity);
        entity_commands.insert(view_point_cloud_bind_groups);
    }
}

/// Prepare a visible nodes texture for given visible nodes `visible_nodes`
fn prepare_visible_nodes_texture(
    commands: &mut Commands<'_, '_>,
    render_point_cloud_chunks: &RenderAssets<RenderPointCloudChunk>,
    render_meshes: &RenderAssets<RenderMesh>,
    mesh_allocator: &MeshAllocator,
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
    let texture = {
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
        let mut sorted_octree_nodes = visible_point_cloud.chunk_entities.clone();

        // remove the missing nodes or unallocated nodes
        sorted_octree_nodes.retain(|node| {
            let Some(render_chunk) = render_point_cloud_chunks.get(node.chunk_id) else {
                info!("render_chunk not yet available");
                return false;
            };

            let Some(mesh_handle) = render_chunk.mesh.as_ref() else {
                info!("mesh not yet available");
                return false;
            };

            if render_meshes.get(mesh_handle.id()).is_none() {
                info!("render mesh not yet available");
                return false;
            };

            // TODO: is it needed to check this far?
            if mesh_allocator
                .mesh_vertex_slice(&mesh_handle.id())
                .is_none()
            {
                info!("mesh_vertex_slice not yet available");
                return false;
            };

            true
        });

        sorted_octree_nodes
            .sort_unstable_by(|a, b| a.depth.cmp(&b.depth).then_with(|| a.name.cmp(&b.name)));

        // this index will contain, for each added chunk's node id, its index in the render
        // visible chunks
        let mut render_node_index = HashMap::<NodeId, usize>::new();

        let octree_index = render_octree_index
            .get(visible_point_cloud.entity)
            .expect("octree index out of bounds")
            .index() as usize;

        let node_mapping = &mut node_index[octree_index];

        // let mut chunk_entities: Vec<VisibleOctreeNodeUniform> = Vec::new();
        let base_offset = octree_index * MAX_NODES;

        for (i, node_entity) in sorted_octree_nodes.into_iter().enumerate() {
            if i >= MAX_NODES {
                // warn!("Too many nodes in octree, some will be ignored.");
                break;
            }

            // we store the future index of the node
            render_node_index.insert(node_entity.id, i);

            // if there is a parent, update its children_mask / children array
            if let Some(parent_node_id) = node_entity.parent_id
                && let Some(&parent_index) = render_node_index.get(&parent_node_id)
            {
                let parent_chunk = &mut visible_nodes_buffer[base_offset + parent_index];

                parent_chunk.children_mask |= node_entity.child_index.mask().bits();
                let current_index = i as u16;
                if current_index < parent_chunk.first_child_index {
                    parent_chunk.first_child_index = current_index;
                }
            }

            // transform offset in u8
            let offset = ((node_entity.offset.unwrap_or(0.0) + 10.0) * 10.0).min(255.0) as u8;

            // insert the visible chunk in
            visible_nodes_buffer[base_offset + i] = VisibleOctreeNodeUniform {
                children_mask: 0,
                offset,
                first_child_index: u16::MAX,
            };

            node_mapping.insert(node_entity.id, i as u32);
        }
    }
    render_queue.write_texture(
        texture.texture.as_image_copy(),
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
        texture,
        node_index,
    });
}
