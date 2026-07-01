use bevy::{
    ecs::{
        query::ROQueryItem,
        system::{
            lifetimeless::{Read, SRes},
            SystemParamItem,
        },
    },
    log::warn,
    pbr::{RenderMeshInstances, SetMeshViewBindGroup, SetMeshViewBindingArrayBindGroup},
    render::{
        erased_render_asset::ErasedRenderAssets,
        mesh::{allocator::MeshAllocator, RenderMesh, RenderMeshBufferInfo},
        render_asset::RenderAssets,
        render_phase::{
            PhaseItem, RenderCommand, RenderCommandResult, SetItemPipeline, TrackedRenderPass,
        },
    },
};

use crate::{PreparedMaterial, PreparedPointCloudUniform, RenderMaterialInstances};

pub type DrawPointCloud = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshViewBindingArrayBindGroup<1>,
    SetPointCloudUniformGroup<2>,
    DrawPointCloudInstanced,
);

// TODO fix
pub type DrawPointCloudPrepass = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    DrawPointCloudInstanced,
);

// TODO fix
pub type DrawPointCloudDepthOnlyPrepass = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    DrawPointCloudInstanced,
);

pub struct SetPointCloudUniformGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetPointCloudUniformGroup<I> {
    type Param = ();
    type ViewQuery = ();
    type ItemQuery = Read<PreparedPointCloudUniform>;

    fn render<'w>(
        _item: &P,
        _view: ROQueryItem<'w, '_, Self::ViewQuery>,
        prepared_custom_uniform: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        _param: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(prepared_point_cloud_uniform) = prepared_custom_uniform else {
            warn!("prepared_point_cloud_uniform missing");
            return RenderCommandResult::Skip;
        };

        pass.set_bind_group(I, &prepared_point_cloud_uniform.bind_group, &[]);

        RenderCommandResult::Success
    }
}

pub struct DrawPointCloudInstanced;

impl<P: PhaseItem> RenderCommand<P> for DrawPointCloudInstanced {
    type Param = (
        SRes<RenderAssets<RenderMesh>>,
        SRes<RenderMeshInstances>,
        SRes<MeshAllocator>,
        SRes<ErasedRenderAssets<PreparedMaterial>>,
        SRes<RenderMaterialInstances>,
    );
    type ViewQuery = ();
    type ItemQuery = ();

    fn render<'w>(
        item: &P,
        _view: (),
        _item_query: Option<()>,
        (meshes, mesh_instances, mesh_allocator, materials, material_instances): SystemParamItem<
            'w,
            '_,
            Self::Param,
        >,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let mesh_instances = mesh_instances.into_inner();
        let mesh_allocator = mesh_allocator.into_inner();
        let materials = materials.into_inner();
        let material_instances = material_instances.into_inner();

        let Some(material_instance) = material_instances.instances.get(&item.main_entity()) else {
            info!("missing material 1");
            return RenderCommandResult::Skip;
        };

        let Some(material) = materials.get(material_instance.asset_id) else {
            info!("missing material 3");

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

        let Some(mesh_asset_id) = mesh_instances.mesh_asset_id(item.main_entity()) else {
            return RenderCommandResult::Skip;
        };
        let Some(vertex_buffer_slice) = mesh_allocator.mesh_vertex_slice(&mesh_asset_id) else {
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
