use std::marker::PhantomData;

use bevy_asset::AssetId;
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
            buffer::ErasedRenderOctreesBuffers,
            components::{RenderOctreeEntityUniform, RenderVisibleOctreeNodes},
            resources::ErasedRenderOctrees,
        },
        visibility::components::VisibleOctreeNode,
    },
    point::Point,
    pointcloud_octree::{
        asset::data::PointCloudNodeData, component::PointCloudOctree3d,
        extract::ErasedRenderPointCloudNode,
    },
    render::mesh::PointCloudMesh,
};

pub struct DrawPointCloudOctreeNode<T: Point>(PhantomData<fn() -> T>);

impl<T: Point> Default for DrawPointCloudOctreeNode<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<P: BinnedPhaseItem, T: Point> RenderCommand<P> for DrawPointCloudOctreeNode<T> {
    type Param = (
        SRes<PointCloudMesh>,
        SRes<ErasedRenderOctrees<ErasedRenderPointCloudNode>>,
    );
    type ViewQuery = Read<RenderVisibleOctreeNodes<PointCloudNodeData<T>, PointCloudOctree3d<T>>>;
    type ItemQuery = Read<PointCloudOctree3d<T>>;

    #[inline]
    fn render<'w>(
        item: &P,
        visible_octree_nodes: &RenderVisibleOctreeNodes<
            PointCloudNodeData<T>,
            PointCloudOctree3d<T>,
        >,
        point_cloud_octree_3d: Option<&PointCloudOctree3d<T>>,
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

        let asset_id: AssetId<_> = point_cloud_octree_3d.into();
        let Some(octree) = render_octrees.get(asset_id) else {
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

pub struct DrawPointCloudOctree<T: Point>(PhantomData<fn() -> T>);

impl<T: Point> Default for DrawPointCloudOctree<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<P: BinnedPhaseItem, T: Point> RenderCommand<P> for DrawPointCloudOctree<T> {
    type Param = (
        SRes<PointCloudMesh>,
        SRes<ErasedRenderOctrees<ErasedRenderPointCloudNode>>,
        SRes<ErasedRenderOctreesBuffers<ErasedRenderPointCloudNode>>,
    );
    type ViewQuery = Read<RenderVisibleOctreeNodes<PointCloudNodeData<T>, PointCloudOctree3d<T>>>;
    type ItemQuery = Read<PointCloudOctree3d<T>>;

    #[inline]
    fn render<'w>(
        item: &P,
        visible_octree_nodes: &RenderVisibleOctreeNodes<
            PointCloudNodeData<T>,
            PointCloudOctree3d<T>,
        >,
        point_cloud_octree_3d: Option<&PointCloudOctree3d<T>>,
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

        let asset_id: AssetId<_> = point_cloud_octree_3d.into();
        let Some(render_octree) = render_octrees.get(asset_id) else {
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
pub struct DrawPointCloudOctreeIndirect<T: Point>(PhantomData<fn() -> T>);

#[cfg(not(feature = "webgl"))]
impl<T: Point> Default for DrawPointCloudOctreeIndirect<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

#[cfg(not(feature = "webgl"))]
impl<P: BinnedPhaseItem, T: Point> RenderCommand<P> for DrawPointCloudOctreeIndirect<T> {
    type Param = (
        SRes<PointCloudMesh>,
        SRes<ErasedRenderOctreesBuffers<ErasedRenderPointCloudNode>>,
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

pub struct SetPointCloudOctreeNodeUniformGroup<const I: usize, T: Point>(PhantomData<fn() -> T>);

impl<const I: usize, T: Point> Default for SetPointCloudOctreeNodeUniformGroup<I, T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<P: BinnedPhaseItem, const I: usize, T: Point> RenderCommand<P>
    for SetPointCloudOctreeNodeUniformGroup<I, T>
{
    type Param = (
        SRes<ErasedRenderOctrees<ErasedRenderPointCloudNode>>,
        SRes<RenderQueue>,
    );
    type ViewQuery = Read<RenderVisibleOctreeNodes<PointCloudNodeData<T>, PointCloudOctree3d<T>>>;
    type ItemQuery = Read<PointCloudOctree3d<T>>;

    fn render<'w>(
        item: &P,
        visible_octree_nodes: &RenderVisibleOctreeNodes<
            PointCloudNodeData<T>,
            PointCloudOctree3d<T>,
        >,
        point_cloud_octree_3d: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        (render_octrees, _render_queue): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let render_octrees = render_octrees.into_inner();

        let Some(point_cloud_octree_3d) = point_cloud_octree_3d else {
            warn!("Missing point cloud octree 3d item");
            return RenderCommandResult::Skip;
        };

        let asset_id: AssetId<_> = point_cloud_octree_3d.into();
        let Some(octree) = render_octrees.get(asset_id) else {
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

pub struct SetRenderOctreeUniformGroup<const I: usize, T: Point>(PhantomData<fn() -> T>);

impl<const I: usize, T: Point> Default for SetRenderOctreeUniformGroup<I, T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<P: BinnedPhaseItem, const I: usize, T: Point> RenderCommand<P>
    for SetRenderOctreeUniformGroup<I, T>
{
    type Param = ();
    type ViewQuery = ();
    type ItemQuery = Read<RenderOctreeEntityUniform<PointCloudNodeData<T>, PointCloudOctree3d<T>>>;

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
