use bevy::{
    ecs::system::{
        lifetimeless::{Read, SRes},
        SystemParamItem,
    },
    pbr::SetMeshViewBindGroup,
    render::{
        mesh::{allocator::MeshAllocator, RenderMesh, RenderMeshBufferInfo},
        render_asset::RenderAssets,
        render_phase::{
            PhaseItem, RenderCommand, RenderCommandResult, SetItemPipeline, TrackedRenderPass,
        },
    },
};

use crate::{PointCloudChunk3d, PointMeshes, RenderPointCloudChunk};

pub type DrawPointCloud = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    DrawPointCloudInstanced,
);

pub struct DrawPointCloudInstanced;

impl<P: PhaseItem> RenderCommand<P> for DrawPointCloudInstanced {
    type Param = (
        SRes<RenderAssets<RenderPointCloudChunk>>,
        SRes<RenderAssets<RenderMesh>>,
        SRes<MeshAllocator>,
        SRes<PointMeshes>,
    );
    type ViewQuery = ();
    type ItemQuery = Read<PointCloudChunk3d>;

    fn render<'w>(
        _item: &P,
        _view: (),
        point_cloud: Option<&'w PointCloudChunk3d>,
        (chunks, meshes, mesh_allocator, point_meshes): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let mesh_allocator = mesh_allocator.into_inner();

        let Some(point_cloud) = point_cloud else {
            return RenderCommandResult::Failure("point cloud missing");
        };

        let Some(quad_mesh) = meshes.get(&point_meshes.quad_mesh) else {
            return RenderCommandResult::Failure("quad missing");
        };
        let Some(quad_vertex_buffer_slice) =
            mesh_allocator.mesh_vertex_slice(&point_meshes.quad_mesh.id())
        else {
            return RenderCommandResult::Failure("unable to get quad vertex slice");
        };

        let Some(chunk) = chunks.get(point_cloud) else {
            return RenderCommandResult::Failure("chunk missing");
        };

        let Some(mesh_handle) = &chunk.mesh else {
            return RenderCommandResult::Failure("mesh missing in chunk");
        };

        let Some(points_mesh) = meshes.get(mesh_handle.id()) else {
            return RenderCommandResult::Failure("points missing");
        };
        let Some(points_vertex_buffer_slice) = mesh_allocator.mesh_vertex_slice(&mesh_handle.id())
        else {
            return RenderCommandResult::Failure("unable to get points vertex slice");
        };

        pass.set_vertex_buffer(0, quad_vertex_buffer_slice.buffer.slice(..));
        pass.set_vertex_buffer(1, points_vertex_buffer_slice.buffer.slice(..));

        match &quad_mesh.buffer_info {
            RenderMeshBufferInfo::Indexed {
                count,
                index_format,
            } => {
                let Some(index_buffer_slice) =
                    mesh_allocator.mesh_index_slice(&point_meshes.quad_mesh.id())
                else {
                    return RenderCommandResult::Skip;
                };

                pass.set_index_buffer(index_buffer_slice.buffer.slice(..), *index_format);
                pass.draw_indexed(
                    index_buffer_slice.range.start..(index_buffer_slice.range.start + count),
                    quad_vertex_buffer_slice.range.start as i32,
                    0..points_mesh.vertex_count,
                );
            }
            RenderMeshBufferInfo::NonIndexed => {
                pass.draw(quad_vertex_buffer_slice.range, 0..points_mesh.vertex_count);
            }
        }

        RenderCommandResult::Success
    }
}
