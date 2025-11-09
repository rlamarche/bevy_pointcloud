use crate::octree::visibility::prepare::RenderOctrees;
use crate::pointcloud_octree::component::PointCloudOctree3d;
use crate::pointcloud_octree::extract::RenderPointCloudNodeData;
use crate::pointcloud_octree::render::data::SetPointCloudOctree3dUniformGroup;
use crate::pointcloud_octree::render::phase::PointCloudOctree3dPhase;
use crate::render::attribute_pass::texture::SetAttributePassTextures;
use crate::render::material::SetPointCloudMaterialGroup;
use crate::render::mesh::PointCloudMesh;
use bevy_ecs::system::{lifetimeless::*, SystemParamItem};
use bevy_log::prelude::*;
use bevy_pbr::{MeshPipeline, SetMeshBindGroup, SetMeshViewBindGroup};
use bevy_render::render_phase::{
    PhaseItem, RenderCommand, RenderCommandResult, SetItemPipeline, TrackedRenderPass,
};
use bevy_render::render_resource::IndexFormat;

pub struct DrawPointCloudOctreeNode;

impl<P: PhaseItem + PointCloudOctree3dPhase> RenderCommand<P> for DrawPointCloudOctreeNode {
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
