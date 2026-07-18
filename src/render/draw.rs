use std::any::TypeId;

use bevy::{
    core_pipeline::prepass::MotionVectorPrepass,
    ecs::{
        query::{Has, ROQueryItem},
        system::{lifetimeless::SRes, SystemParamItem},
    },
    log::warn,
    pbr::{MeshBindGroups, MeshMorphBindGroupKey, MorphIndices, RenderMeshInstances, SkinUniforms},
    render::{
        erased_render_asset::ErasedRenderAssets,
        mesh::{allocator::MeshAllocator, RenderMesh, RenderMeshBufferInfo},
        render_asset::RenderAssets,
        render_phase::{
            PhaseItem, PhaseItemExtraIndex, RenderCommand, RenderCommandResult, TrackedRenderPass,
        },
        renderer::RenderDevice,
    },
};

use crate::{
    skins_use_uniform_buffers, PreparedMaterial, PreparedPointCloudUniforms,
    RenderPointCloudChunkInstances, RenderPointCloudMaterialInstances,
};

/// Same as [`bevy::pbr::SetMeshBindGroup`] but use root point cloud mesh.
pub struct SetMeshBindGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetMeshBindGroup<I> {
    type Param = (
        SRes<RenderDevice>,
        SRes<MeshBindGroups>,
        SRes<RenderPointCloudChunkInstances>,
        SRes<RenderMeshInstances>,
        SRes<SkinUniforms>,
        SRes<MorphIndices>,
        SRes<MeshAllocator>,
    );
    type ViewQuery = Has<MotionVectorPrepass>;
    type ItemQuery = ();

    #[inline]
    fn render<'w>(
        item: &P,
        has_motion_vector_prepass: bool,
        _item_query: Option<()>,
        (
            render_device,
            bind_groups,
            render_point_cloud_chunk_instances,
            mesh_instances,
            skin_uniforms,
            morph_indices,
            mesh_allocator,
        ): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let bind_groups = bind_groups.into_inner();
        let mesh_instances = mesh_instances.into_inner();
        let skin_uniforms = skin_uniforms.into_inner();
        let morph_indices = morph_indices.into_inner();

        let entity = &item.main_entity();

        let Some(chunk_instance) = render_point_cloud_chunk_instances.get(&item.entity()) else {
            warn!(
                "render_point_cloud_chunk_instance missing 1 for entity {:?} main {:?}",
                item.entity(),
                item.main_entity()
            );
            return RenderCommandResult::Skip;
        };

        // get mesh instance from the root
        let Some(mesh_asset_id) = mesh_instances.mesh_asset_id(chunk_instance.root_entity) else {
            return RenderCommandResult::Success;
        };

        let skins_use_uniform_buffers = skins_use_uniform_buffers(&render_device.limits());

        let current_skin_byte_offset = skin_uniforms.skin_byte_offset(*entity);

        // Determine which morph bind group key we need, if any. If the platform
        // doesn't support storage buffers, there's a separate bind group per
        // mesh. Otherwise, if the platform does support storage buffers,
        // there's one bind group per morph target displacement slab (managed by
        // the mesh allocator).
        let (current_morph_index, prev_morph_index, morph_bind_group_key);
        match *morph_indices {
            MorphIndices::Uniform {
                ref current,
                ref prev,
            } => {
                current_morph_index = current.get(entity);
                prev_morph_index = prev.get(entity);
                morph_bind_group_key = if current_morph_index.is_some() {
                    MeshMorphBindGroupKey::Uniform(mesh_asset_id)
                } else {
                    MeshMorphBindGroupKey::NoMorphTargets
                };
            }
            MorphIndices::Storage { .. } => {
                current_morph_index = None;
                prev_morph_index = None;
                morph_bind_group_key = match mesh_allocator
                    .mesh_slabs(&mesh_asset_id)
                    .and_then(|mesh_slabs| mesh_slabs.morph_target_slab_id)
                {
                    Some(morph_target_slab_id) => {
                        MeshMorphBindGroupKey::Storage(morph_target_slab_id)
                    }
                    None => MeshMorphBindGroupKey::NoMorphTargets,
                };
            }
        };

        let is_skinned = current_skin_byte_offset.is_some();

        // TODO: make this public in Bevy ?
        // let lightmap_slab_index = lightmaps
        //     .render_lightmaps
        //     .get(entity)
        //     .map(|render_lightmap| render_lightmap.slab_index);

        let Some(mesh_phase_bind_groups) = (match *bind_groups {
            MeshBindGroups::CpuPreprocessing(ref mesh_phase_bind_groups) => {
                Some(mesh_phase_bind_groups)
            }
            MeshBindGroups::GpuPreprocessing(ref mesh_phase_bind_groups) => {
                mesh_phase_bind_groups.get(&TypeId::of::<P>())
            }
        }) else {
            // This is harmless if e.g. we're rendering the `Shadow` phase and
            // there weren't any shadows.
            return RenderCommandResult::Success;
        };

        let Some(bind_group) = mesh_phase_bind_groups.get(
            // lightmap_slab_index,
            None,
            is_skinned,
            morph_bind_group_key,
            has_motion_vector_prepass,
        ) else {
            warn!(
                "The MeshBindGroups resource wasn't set in the render phase. \
            It should be set by the prepare_mesh_bind_group system.\n\
            This is a bevy bug! Please open an issue."
            );
            return RenderCommandResult::Failure(
                "The MeshBindGroups resource wasn't set in the render phase. \
                It should be set by the prepare_mesh_bind_group system.\n\
                This is a bevy bug! Please open an issue.",
            );
        };

        let mut dynamic_offsets: [u32; 5] = Default::default();
        let mut offset_count = 0;
        if let PhaseItemExtraIndex::DynamicOffset(dynamic_offset) = item.extra_index() {
            dynamic_offsets[offset_count] = dynamic_offset;
            offset_count += 1;
        }
        if skins_use_uniform_buffers {
            if let Some(current_skin_index) = current_skin_byte_offset {
                dynamic_offsets[offset_count] = current_skin_index.byte_offset;
                offset_count += 1;
            }
            if let Some(current_morph_index) = current_morph_index {
                dynamic_offsets[offset_count] = current_morph_index.index;
                offset_count += 1;
            }
        }

        // Attach motion vectors if needed.
        if skins_use_uniform_buffers && has_motion_vector_prepass {
            // Attach the previous skin index for motion vector computation.
            if let Some(current_skin_byte_offset) = current_skin_byte_offset {
                dynamic_offsets[offset_count] = current_skin_byte_offset.byte_offset;
                offset_count += 1;
            }

            // Attach the previous morph index for motion vector computation. If
            // there isn't one, just use zero as the shader will ignore it.
            if current_morph_index.is_some() {
                match prev_morph_index {
                    Some(prev_morph_index) => {
                        dynamic_offsets[offset_count] = prev_morph_index.index;
                    }
                    None => dynamic_offsets[offset_count] = 0,
                }
                offset_count += 1;
            }
        }

        pass.set_bind_group(I, bind_group, &dynamic_offsets[0..offset_count]);

        RenderCommandResult::Success
    }
}

pub struct SetPointCloudUniformGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetPointCloudUniformGroup<I> {
    type Param = (
        SRes<RenderPointCloudChunkInstances>,
        SRes<PreparedPointCloudUniforms>,
    );
    type ViewQuery = ();
    type ItemQuery = ();

    fn render<'w>(
        item: &P,
        _view: ROQueryItem<'w, '_, Self::ViewQuery>,
        _: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        (render_point_cloud_chunk_instances, prepared_point_cloud_uniforms): SystemParamItem<
            'w,
            '_,
            Self::Param,
        >,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let prepared_point_cloud_uniforms = prepared_point_cloud_uniforms.into_inner();

        let Some(chunk_instance) = render_point_cloud_chunk_instances.get(&item.entity()) else {
            warn!("render_point_cloud_chunk_instance missing 2");
            return RenderCommandResult::Skip;
        };

        let Some(prepared_point_cloud_uniform) =
            prepared_point_cloud_uniforms.get(&chunk_instance.root_entity)
        else {
            warn!(
                "prepared_point_cloud_uniform missing for root entity {:?}",
                chunk_instance.root_entity
            );
            return RenderCommandResult::Skip;
        };

        pass.set_bind_group(I, &prepared_point_cloud_uniform.bind_group, &[]);

        RenderCommandResult::Success
    }
}

pub struct DrawPointCloudInstanced;

impl<P: PhaseItem> RenderCommand<P> for DrawPointCloudInstanced {
    type Param = (
        SRes<RenderPointCloudChunkInstances>,
        SRes<RenderAssets<RenderMesh>>,
        SRes<MeshAllocator>,
        SRes<ErasedRenderAssets<PreparedMaterial>>,
        SRes<RenderPointCloudMaterialInstances>,
    );
    type ViewQuery = ();
    type ItemQuery = ();

    fn render<'w>(
        item: &P,
        _view: (),
        _item_query: Option<()>,
        (
            render_point_cloud_chunk_instances,
            meshes,
            mesh_allocator,
            materials,
            material_instances,
        ): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let mesh_allocator = mesh_allocator.into_inner();
        let materials = materials.into_inner();
        let material_instances = material_instances.into_inner();

        let Some(chunk_instance) = render_point_cloud_chunk_instances.get(&item.entity()) else {
            warn!("render_point_cloud_chunk_instance missing 3");
            return RenderCommandResult::Skip;
        };

        let Some(material_instance) = material_instances
            .instances
            .get(&chunk_instance.root_entity)
        else {
            warn!(
                "material_instance not found for entity {:?}",
                item.main_entity()
            );
            return RenderCommandResult::Skip;
        };

        let Some(material) = materials.get(material_instance.asset_id) else {
            warn!("material not found for entity {:?}", item.main_entity());
            return RenderCommandResult::Skip;
        };

        let shape_mesh_id = material.properties.shape_mesh;

        let Some(shape_mesh) = meshes.get(shape_mesh_id) else {
            return RenderCommandResult::Failure("quad missing");
        };

        let Some(quad_vertex_buffer_slice) = mesh_allocator.mesh_vertex_slice(&shape_mesh_id)
        else {
            return RenderCommandResult::Failure("unable to get quad vertex slice");
        };

        // let Some(mesh_asset_id) = mesh_instances.mesh_asset_id(item.main_entity()) else {
        //     return RenderCommandResult::Skip;
        // };
        let Some(vertex_buffer_slice) =
            mesh_allocator.mesh_vertex_slice(&chunk_instance.mesh_asset_id)
        else {
            warn!("vertex buffer slice not found");
            return RenderCommandResult::Skip;
        };

        pass.set_vertex_buffer(0, quad_vertex_buffer_slice.buffer.slice(..));
        pass.set_vertex_buffer(1, vertex_buffer_slice.buffer.slice(..));

        match &shape_mesh.buffer_info {
            RenderMeshBufferInfo::Indexed {
                count,
                index_format,
            } => {
                let Some(index_buffer_slice) = mesh_allocator.mesh_index_slice(&shape_mesh_id)
                else {
                    warn!("index_buffer_slice slice not found for shape");
                    return RenderCommandResult::Skip;
                };

                pass.set_index_buffer(index_buffer_slice.buffer.slice(..), *index_format);

                pass.draw_indexed(
                    index_buffer_slice.range.start..(index_buffer_slice.range.start + count),
                    quad_vertex_buffer_slice.range.start as i32,
                    vertex_buffer_slice.range,
                );
            }
            RenderMeshBufferInfo::NonIndexed => {
                pass.draw(quad_vertex_buffer_slice.range, vertex_buffer_slice.range);
            }
        }

        RenderCommandResult::Success
    }
}
