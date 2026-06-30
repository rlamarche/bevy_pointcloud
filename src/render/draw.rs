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
        mesh::{allocator::MeshAllocator, RenderMesh, RenderMeshBufferInfo},
        render_asset::RenderAssets,
        render_phase::{
            PhaseItem, RenderCommand, RenderCommandResult, SetItemPipeline, TrackedRenderPass,
        },
    },
};

use crate::{PreparedPointCloudUniform, ShapeMeshes};

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
        SRes<ShapeMeshes>,
    );
    type ViewQuery = ();
    type ItemQuery = ();

    fn render<'w>(
        item: &P,
        _view: (),
        _item_query: Option<()>,
        (meshes, mesh_instances, mesh_allocator, shape_meshes): SystemParamItem<
            'w,
            '_,
            Self::Param,
        >,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let meshes = meshes.into_inner();
        let mesh_instances = mesh_instances.into_inner();
        let mesh_allocator = mesh_allocator.into_inner();

        let Some(mesh_asset_id) = mesh_instances.mesh_asset_id(item.main_entity()) else {
            return RenderCommandResult::Skip;
        };
        let Some(vertex_buffer_slice) = mesh_allocator.mesh_vertex_slice(&mesh_asset_id) else {
            return RenderCommandResult::Skip;
        };

        // TODO load from a configuration ?
        let Some(quad_mesh) = meshes.get(&shape_meshes.quad_mesh) else {
            return RenderCommandResult::Failure("quad missing");
        };
        let Some(quad_vertex_buffer_slice) =
            mesh_allocator.mesh_vertex_slice(&shape_meshes.quad_mesh.id())
        else {
            return RenderCommandResult::Failure("unable to get quad vertex slice");
        };

        pass.set_vertex_buffer(0, quad_vertex_buffer_slice.buffer.slice(..));
        pass.set_vertex_buffer(1, vertex_buffer_slice.buffer.slice(..));

        match &quad_mesh.buffer_info {
            RenderMeshBufferInfo::Indexed {
                count,
                index_format,
            } => {
                let Some(index_buffer_slice) =
                    mesh_allocator.mesh_index_slice(&shape_meshes.quad_mesh.id())
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
