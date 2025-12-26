use super::phase::PointCloudOctree3dPhase;
use crate::octree::new_asset::extract::resources::RenderOctrees;
use crate::pointcloud_octree::extract::RenderPointCloudNodeData;
use crate::pointcloud_octree::new_asset::component::NewPointCloudOctree3d;
use crate::render::mesh::PointCloudMesh;
use bevy_ecs::query::ROQueryItem;
use bevy_ecs::system::{lifetimeless::*, SystemParamItem};
use bevy_log::prelude::*;
use bevy_render::render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass};
use bevy_render::render_resource::IndexFormat;
use bevy_render::renderer::RenderQueue;

pub struct DrawPointCloudOctreeNode;

impl<P: PhaseItem + PointCloudOctree3dPhase> RenderCommand<P> for DrawPointCloudOctreeNode {
    type Param = (
        SRes<PointCloudMesh>,
        SRes<RenderOctrees<RenderPointCloudNodeData>>,
    );
    type ViewQuery = ();
    type ItemQuery = Read<NewPointCloudOctree3d>;

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        point_cloud_octree_3d: Option<&NewPointCloudOctree3d>,
        (point_cloud_mesh, render_octrees): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        // A borrow check workaround.
        let point_cloud_mesh = point_cloud_mesh.into_inner();
        let render_octrees = render_octrees.into_inner();

        let Some(point_cloud_octree_3d) = point_cloud_octree_3d else {
            warn!("Missing point cloud octree 3d item");
            return RenderCommandResult::Skip;
        };

        let Some(octree) = render_octrees.get(point_cloud_octree_3d) else {
            warn!("Missing octree when render");
            return RenderCommandResult::Skip;
        };

        let node_id = item.node_id();

        let Some(node) = octree.nodes.get(&node_id) else {
            warn!("Missing node when render");
            return RenderCommandResult::Skip;
        };

        pass.set_vertex_buffer(0, point_cloud_mesh.vertex_buffer.slice(..));
        pass.set_index_buffer(
            point_cloud_mesh.index_buffer.slice(..),
            0,
            IndexFormat::Uint32,
        );

        pass.set_vertex_buffer(1, node.data.points.slice(..));

        pass.draw_indexed(
            0..point_cloud_mesh.index_count,
            0,
            0..node.data.num_points as u32,
        );

        RenderCommandResult::Success
    }
}

pub struct SetPointCloudOctreeNodeUniformGroup<const I: usize>;
impl<P: PhaseItem + PointCloudOctree3dPhase, const I: usize> RenderCommand<P>
    for SetPointCloudOctreeNodeUniformGroup<I>
{
    type Param = (
        SRes<RenderOctrees<RenderPointCloudNodeData>>,
        SRes<RenderQueue>,
    );
    type ViewQuery = ();
    type ItemQuery = Read<NewPointCloudOctree3d>;

    fn render<'w>(
        item: &P,
        _: ROQueryItem<'w, '_, Self::ViewQuery>,
        point_cloud_octree_3d: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        (render_octrees, render_queue): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let render_octrees = render_octrees.into_inner();

        let Some(point_cloud_octree_3d) = point_cloud_octree_3d else {
            warn!("Missing point cloud octree 3d item");
            return RenderCommandResult::Skip;
        };

        let Some(octree) = render_octrees.get(point_cloud_octree_3d) else {
            warn!("Missing octree when render");
            return RenderCommandResult::Skip;
        };

        let node_id = item.node_id();

        let Some(node) = octree.nodes.get(&node_id) else {
            warn!("Missing node when render");
            return RenderCommandResult::Skip;
        };

        pass.set_bind_group(I, &node.data.uniform, &[]);

        RenderCommandResult::Success
    }
}
