use bevy::{
    ecs::system::{lifetimeless::SRes, SystemParamItem},
    pbr::{RenderMeshInstances, SetMeshViewBindGroup},
    render::{
        mesh::{allocator::MeshAllocator, RenderMesh, RenderMeshBufferInfo},
        render_asset::RenderAssets,
        render_phase::{
            PhaseItem, RenderCommand, RenderCommandResult, SetItemPipeline, TrackedRenderPass,
        },
    },
};

use crate::ShapeMeshes;

pub type DrawPointCloud = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
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
        let Some(gpu_mesh) = meshes.get(mesh_asset_id) else {
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
                    0..gpu_mesh.vertex_count,
                );
            }
            RenderMeshBufferInfo::NonIndexed => {
                pass.draw(quad_vertex_buffer_slice.range, 0..gpu_mesh.vertex_count);
            }
        }

        RenderCommandResult::Success
    }
}
