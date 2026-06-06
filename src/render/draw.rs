use std::marker::PhantomData;

use bevy_ecs::system::{
    lifetimeless::{Read, SRes},
    SystemParamItem,
};
use bevy_render::{
    render_asset::RenderAssets,
    render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass},
    render_resource::IndexFormat,
};

use crate::{
    point::{GpuPoint, Point},
    point_cloud::PointCloud3d,
    render::{mesh::PointCloudMesh, point_cloud::RenderPointCloud},
};

pub struct DrawPointCloud<T: Point, U: GpuPoint>(PhantomData<fn() -> (T, U)>);

impl<T: Point, U: GpuPoint, P: PhaseItem> RenderCommand<P> for DrawPointCloud<T, U>
where
    for<'a> &'a T: Into<U>,
{
    type Param = (
        SRes<PointCloudMesh>,
        SRes<RenderAssets<RenderPointCloud<T, U>>>,
    );
    type ViewQuery = ();
    type ItemQuery = Read<PointCloud3d<T>>;

    #[inline]
    fn render<'w>(
        _item: &P,
        _view: (),
        point_cloud_3d: Option<&'w PointCloud3d<T>>,
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

        pass.set_vertex_buffer(0, point_cloud_mesh.vertex_buffer.slice(..));
        pass.set_index_buffer(point_cloud_mesh.index_buffer.slice(..), IndexFormat::Uint32);

        pass.set_vertex_buffer(1, render_point_cloud.buffer.slice(..));

        pass.draw_indexed(
            0..point_cloud_mesh.index_count,
            0,
            0..render_point_cloud.length as u32,
        );

        RenderCommandResult::Success
    }
}
