use bevy_ecs::{
    query::ROQueryItem,
    system::{lifetimeless::*, SystemParamItem},
};
use bevy_log::prelude::*;
use bevy_render::{
    render_phase::{BinnedPhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass},
    renderer::RenderQueue,
};

#[cfg(not(feature = "webgl"))]
use crate::pointcloud_octree::render::indirect::RenderVisibleNodesIndirectBuffers;
use crate::{
    octree::{
        extract::render::{
            buffer::RenderOctreesBuffers,
            components::{RenderOctreeEntityUniform, RenderVisibleOctreeNodes},
            resources::RenderOctrees,
        },
        visibility::components::VisibleOctreeNode,
    },
    pointcloud_octree::{
        asset::data::PointCloudNodeData, component::PointCloudOctree3d,
        extract::RenderPointCloudNodeData,
    },
    render::mesh::PointCloudMesh,
};

pub struct DrawPointCloudOctreeNode;

impl<P: BinnedPhaseItem> RenderCommand<P> for DrawPointCloudOctreeNode {
    type Param = (
        SRes<PointCloudMesh>,
        SRes<RenderOctrees<RenderPointCloudNodeData>>,
    );
    type ViewQuery = Read<RenderVisibleOctreeNodes<PointCloudNodeData, PointCloudOctree3d>>;
    type ItemQuery = Read<PointCloudOctree3d>;

    #[inline]
    fn render<'w>(
        item: &P,
        visible_octree_nodes: &RenderVisibleOctreeNodes<PointCloudNodeData, PointCloudOctree3d>,
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
            debug!("Missing octree when render");
            return RenderCommandResult::Skip;
        };

        let Some((_, visible_octree_nodes)) = visible_octree_nodes.octrees.get(&item.entity())
        else {
            warn!("Missing visible octree data");
            return RenderCommandResult::Skip;
        };

        for node in visible_octree_nodes {
            let Some(node) = octree.nodes.get(&node.id) else {
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

impl<P: BinnedPhaseItem> RenderCommand<P> for DrawPointCloudOctree {
    type Param = (
        SRes<PointCloudMesh>,
        SRes<RenderOctrees<RenderPointCloudNodeData>>,
        SRes<RenderOctreesBuffers<RenderPointCloudNodeData>>,
    );
    type ViewQuery = Read<RenderVisibleOctreeNodes<PointCloudNodeData, PointCloudOctree3d>>;
    type ItemQuery = Read<PointCloudOctree3d>;

    #[inline]
    fn render<'w>(
        item: &P,
        visible_octree_nodes: &RenderVisibleOctreeNodes<PointCloudNodeData, PointCloudOctree3d>,
        point_cloud_octree_3d: Option<&PointCloudOctree3d>,
        (point_cloud_mesh, render_octrees, render_octrees_buffers): SystemParamItem<
            'w,
            '_,
            Self::Param,
        >,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        // A borrow check workaround.
        let point_cloud_mesh = point_cloud_mesh.into_inner();
        let render_octrees = render_octrees.into_inner();
        let render_octrees_buffers = render_octrees_buffers.into_inner();

        let Some(point_cloud_octree_3d) = point_cloud_octree_3d else {
            warn!("Missing point cloud octree 3d item");
            return RenderCommandResult::Skip;
        };

        let Some(octrees_buffer) = render_octrees_buffers.get(0) else {
            warn!("Missing octrees buffer when render");
            return RenderCommandResult::Skip;
        };

        let Some(render_octree) = render_octrees.get(point_cloud_octree_3d) else {
            debug!("Missing octree when render");
            return RenderCommandResult::Skip;
        };

        let Some((_, visible_octree_nodes)) = visible_octree_nodes.octrees.get(&item.entity())
        else {
            warn!("Missing visible octree data");
            return RenderCommandResult::Skip;
        };

        pass.set_vertex_buffer(0, point_cloud_mesh.vertex_buffer.slice(..));
        // not needed is using a single triangle
        // pass.set_index_buffer(
        //     point_cloud_mesh.index_buffer.slice(..),
        //     0,
        //     IndexFormat::Uint32,
        // );

        pass.set_vertex_buffer(1, octrees_buffer.buffer.slice(..));

        // not needed if using a single triangle
        // pass.draw_indexed(
        //     0..point_cloud_mesh.index_count,
        //     0,
        //     0..node.data.num_points as u32,
        // );

        for VisibleOctreeNode { id: node_id, .. } in visible_octree_nodes {
            if let Some(render_octree_node_data) = render_octree.nodes.get(node_id) {
                pass.draw(
                    0..point_cloud_mesh.index_count,
                    render_octree_node_data.allocation.start
                        ..(render_octree_node_data.allocation.start
                            + render_octree_node_data.allocation.count),
                );
            }
        }

        RenderCommandResult::Success
    }
}

#[cfg(not(feature = "webgl"))]
pub struct DrawPointCloudOctreeIndirect;

#[cfg(not(feature = "webgl"))]
impl<P: BinnedPhaseItem> RenderCommand<P> for DrawPointCloudOctreeIndirect {
    type Param = (
        SRes<PointCloudMesh>,
        SRes<RenderOctreesBuffers<RenderPointCloudNodeData>>,
    );
    type ViewQuery = Read<RenderVisibleNodesIndirectBuffers>;
    type ItemQuery = ();

    #[inline]
    fn render<'w>(
        item: &P,
        render_visible_nodes_indirect_buffers: ROQueryItem<'w, '_, Self::ViewQuery>,
        _: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        (point_cloud_mesh, render_octrees_buffers): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        // A borrow check workaround.
        let point_cloud_mesh = point_cloud_mesh.into_inner();
        let render_octrees_buffers = render_octrees_buffers.into_inner();

        let Some(octrees_buffer) = render_octrees_buffers.get(0) else {
            warn!("Missing octrees buffer when render");
            return RenderCommandResult::Skip;
        };

        let Some(indirect_buffer) = render_visible_nodes_indirect_buffers.get(&item.entity())
        else {
            warn!("Missing visible octree data");
            return RenderCommandResult::Skip;
        };

        let Some(buffer) = indirect_buffer.buffer() else {
            warn!("Missing indirect buffer");
            return RenderCommandResult::Skip;
        };

        pass.set_vertex_buffer(0, point_cloud_mesh.vertex_buffer.slice(..));
        // not needed is using a single triangle
        // pass.set_index_buffer(
        //     point_cloud_mesh.index_buffer.slice(..),
        //     0,
        //     IndexFormat::Uint32,
        // );

        pass.set_vertex_buffer(1, octrees_buffer.buffer.slice(..));

        // not needed if using a single triangle
        // pass.draw_indexed(
        //     0..point_cloud_mesh.index_count,
        //     0,
        //     0..node.data.num_points as u32,
        // );

        pass.multi_draw_indirect(buffer, 0, indirect_buffer.len() as u32);

        RenderCommandResult::Success
    }
}

pub struct SetPointCloudOctreeNodeUniformGroup<const I: usize>;
impl<P: BinnedPhaseItem, const I: usize> RenderCommand<P>
    for SetPointCloudOctreeNodeUniformGroup<I>
{
    type Param = (
        SRes<RenderOctrees<RenderPointCloudNodeData>>,
        SRes<RenderQueue>,
    );
    type ViewQuery = Read<RenderVisibleOctreeNodes<PointCloudNodeData, PointCloudOctree3d>>;
    type ItemQuery = Read<PointCloudOctree3d>;

    fn render<'w>(
        item: &P,
        visible_octree_nodes: &RenderVisibleOctreeNodes<PointCloudNodeData, PointCloudOctree3d>,
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
            debug!("Missing octree when render");
            return RenderCommandResult::Skip;
        };

        let Some((_, visible_octree_nodes)) = visible_octree_nodes.octrees.get(&item.entity())
        else {
            warn!("Missing visible octree data");
            return RenderCommandResult::Skip;
        };

        if !visible_octree_nodes.is_empty() {
            let root_node = &visible_octree_nodes[0];

            let Some(node) = octree.nodes.get(&root_node.id) else {
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

pub struct SetRenderOctreeUniformGroup<const I: usize>;
impl<P: BinnedPhaseItem, const I: usize> RenderCommand<P> for SetRenderOctreeUniformGroup<I> {
    type Param = ();
    type ViewQuery = ();
    type ItemQuery = Read<RenderOctreeEntityUniform<PointCloudNodeData, PointCloudOctree3d>>;

    fn render<'w>(
        _item: &P,
        _: ROQueryItem<'w, '_, Self::ViewQuery>,
        render_octree_uniform: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        _: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(render_octree_uniform) = render_octree_uniform else {
            warn!("Missing RenderOctreeUniform item");
            return RenderCommandResult::Skip;
        };

        pass.set_bind_group(I, &render_octree_uniform.bind_group, &[]);

        RenderCommandResult::Success
    }
}
