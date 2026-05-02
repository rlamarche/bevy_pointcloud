use crate::point_cloud::PointCloud3d;
use crate::render::mesh::PointCloudMesh;
use crate::render::point_cloud::RenderPointCloud;
use bevy_ecs::system::lifetimeless::{Read, SRes};
use bevy_ecs::system::SystemParamItem;
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass};
use bevy_render::render_resource::IndexFormat;

pub struct DrawPointCloud;

impl<P: PhaseItem> RenderCommand<P> for DrawPointCloud {
    type Param = (SRes<PointCloudMesh>, SRes<RenderAssets<RenderPointCloud>>);
    type ViewQuery = ();
    type ItemQuery = Read<PointCloud3d>;

    #[inline]
    fn render<'w>(
        _item: &P,
        _view: (),
        point_cloud_3d: Option<&'w PointCloud3d>,
        (point_cloud_mesh, render_point_clouds): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        // A borrow check workaround.
        let point_cloud_mesh = point_cloud_mesh.into_inner();
        let render_point_clouds = render_point_clouds.into_inner();

        let Some(point_cloud_3d) = point_cloud_3d else {
            return RenderCommandResult::Skip;
        };
        let Some(render_point_cloud) = render_point_clouds.get(point_cloud_3d) else {
            return RenderCommandResult::Skip;
        };

        // // Tell the GPU where the vertices are.
        // pass.set_vertex_buffer(
        //     0,
        //     point_cloud_mesh
        //         .vertices
        //         .buffer()
        //         .unwrap()
        //         .slice(..),
        // );
        //
        // // Tell the GPU where the indices are.
        // pass.set_index_buffer(
        //     point_cloud_mesh
        //         .indices
        //         .buffer()
        //         .unwrap()
        //         .slice(..),
        //     0,
        //     IndexFormat::Uint32,
        // );


        pass.set_vertex_buffer(0, point_cloud_mesh.vertex_buffer.slice(..));
        pass.set_index_buffer(
            point_cloud_mesh.index_buffer.slice(..),
            IndexFormat::Uint32,
        );

        pass.set_vertex_buffer(1, render_point_cloud.buffer.slice(..));

        pass.draw_indexed(
            0..point_cloud_mesh.index_count,
            0,
            0..render_point_cloud.length as u32,
        );

        RenderCommandResult::Success
    }
}
