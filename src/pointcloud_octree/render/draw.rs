use crate::octree::extract::buffer::{AllocationInfo, RenderOctreesBuffer};
use crate::octree::extract::resources::RenderOctrees;
use crate::pointcloud_octree::component::PointCloudOctree3d;
use crate::pointcloud_octree::extract::RenderPointCloudNodeData;
use crate::pointcloud_octree::render::phase::PointCloudOctreeBinnedPhaseItem;
use crate::render::mesh::PointCloudMesh;
use bevy_ecs::query::ROQueryItem;
use bevy_ecs::system::{SystemParamItem, lifetimeless::*};
use bevy_log::prelude::*;
use bevy_render::render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass};
use bevy_render::renderer::RenderQueue;

pub struct DrawPointCloudOctreeNode;

impl<P: PointCloudOctreeBinnedPhaseItem> RenderCommand<P> for DrawPointCloudOctreeNode {
    type Param = (
        SRes<PointCloudMesh>,
        SRes<RenderOctrees<RenderPointCloudNodeData>>,
    );
    type ViewQuery = ();
    type ItemQuery = Read<PointCloudOctree3d>;

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        point_cloud_octree_3d: Option<&PointCloudOctree3d>,
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

        let node_ids = item.node_ids();

        for node_id in node_ids {
            let Some(node) = octree.nodes.get(node_id) else {
                warn!("Missing node when render");
                return RenderCommandResult::Skip;
            };

            // not needed is using a single triangle
            // pass.set_index_buffer(
            //     point_cloud_mesh.index_buffer.slice(..),
            //     0,
            //     IndexFormat::Uint32,
            // );

            let Some(points) = &node.data.points else {
                return RenderCommandResult::Skip;
            };

            pass.set_vertex_buffer(0, point_cloud_mesh.vertex_buffer.slice(..));
            pass.set_vertex_buffer(1, points.slice(..));

            // not needed is using a single triangle
            // pass.draw_indexed(
            //     0..point_cloud_mesh.index_count,
            //     0,
            //     0..node.data.num_points as u32,
            // );

            pass.draw(
                0..point_cloud_mesh.index_count,
                0..node.data.num_points as u32,
            );
        }
        RenderCommandResult::Success
    }
}

pub struct DrawPointCloudOctree;

impl<P: PointCloudOctreeBinnedPhaseItem> RenderCommand<P> for DrawPointCloudOctree {
    type Param = (
        SRes<PointCloudMesh>,
        SRes<RenderOctreesBuffer<RenderPointCloudNodeData>>,
    );
    type ViewQuery = ();
    type ItemQuery = Read<PointCloudOctree3d>;

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        point_cloud_octree_3d: Option<&PointCloudOctree3d>,
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

        pass.set_vertex_buffer(0, point_cloud_mesh.vertex_buffer.slice(..));
        // not needed is using a single triangle
        // pass.set_index_buffer(
        //     point_cloud_mesh.index_buffer.slice(..),
        //     0,
        //     IndexFormat::Uint32,
        // );

        pass.set_vertex_buffer(1, octree.buffer.slice(..));

        // not needed is using a single triangle
        // pass.draw_indexed(
        //     0..point_cloud_mesh.index_count,
        //     0,
        //     0..node.data.num_points as u32,
        // );

        for node_id in item.node_ids() {
            if let Some(AllocationInfo { start, end, .. }) = octree.allocation_index.get(node_id) {
                pass.draw(0..point_cloud_mesh.index_count, *start..*end);
            }
        }

        RenderCommandResult::Success
    }
}

pub struct SetPointCloudOctreeNodeUniformGroup<const I: usize>;
impl<P: PhaseItem + PointCloudOctreeBinnedPhaseItem, const I: usize> RenderCommand<P>
    for SetPointCloudOctreeNodeUniformGroup<I>
{
    type Param = (
        SRes<RenderOctrees<RenderPointCloudNodeData>>,
        SRes<RenderQueue>,
    );
    type ViewQuery = ();
    type ItemQuery = Read<PointCloudOctree3d>;

    fn render<'w>(
        item: &P,
        _: ROQueryItem<'w, '_, Self::ViewQuery>,
        point_cloud_octree_3d: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        (render_octrees, _render_queue): SystemParamItem<'w, '_, Self::Param>,
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

        // Bind root node
        let node_ids = item.node_ids();

        if node_ids.len() > 0 {
            let node_id = item.node_ids()[0];

            let Some(node) = octree.nodes.get(&node_id) else {
                warn!("Missing node when render");
                return RenderCommandResult::Skip;
            };

            pass.set_bind_group(I, &node.data.uniform, &[]);
            RenderCommandResult::Success
        } else {
            RenderCommandResult::Skip
        }
    }
}
