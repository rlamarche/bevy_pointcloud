use crate::octree::extract::RenderOctrees;
use crate::pointcloud_octree::component::PointCloudOctree3d;
use crate::pointcloud_octree::extract::RenderPointCloudNodeData;
use crate::pointcloud_octree::render::data::SetPointCloudOctree3dUniformGroup;
use crate::pointcloud_octree::render::mesh::PointCloudOctreeMesh;
use crate::pointcloud_octree::render::phase::PointCloudOctree3dPhase;
use crate::render::attribute_pass::texture::SetAttributePassTextures;
use crate::render::material::SetPointCloudMaterialGroup;
use bevy_asset::AssetId;
use bevy_core_pipeline::prepass::MotionVectorPrepass;
use bevy_ecs::prelude::*;
use bevy_ecs::query::ROQueryItem;
use bevy_ecs::system::{SystemParamItem, lifetimeless::*};
use bevy_log::prelude::*;
use bevy_mesh::Mesh;
use bevy_pbr::{MeshBindGroups, MeshPipeline, SetMeshBindGroup, SetMeshViewBindGroup};
use bevy_render::render_phase::{
    PhaseItem, RenderCommand, RenderCommandResult, SetItemPipeline, TrackedRenderPass,
};
use bevy_render::render_resource::IndexFormat;
use bevy_render::renderer::RenderDevice;
use std::any::TypeId;

pub type DrawPointCloudOctree = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetPointCloudOctree3dUniformGroup<1>,
    SetPointCloudMaterialGroup<2>,
    SetAttributePassTextures<3>,
    DrawPointCloudOctreeNode,
);

pub struct DrawPointCloudOctreeNode;

impl RenderCommand<PointCloudOctree3dPhase> for DrawPointCloudOctreeNode {
    type Param = (
        SRes<PointCloudOctreeMesh>,
        SRes<RenderOctrees<RenderPointCloudNodeData>>,
    );
    type ViewQuery = ();
    type ItemQuery = ();

    #[inline]
    fn render<'w>(
        item: &PointCloudOctree3dPhase,
        _view: (),
        _item: Option<()>,
        (point_cloud_octree_mesh, render_octrees): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        // A borrow check workaround.
        let point_cloud_octree_mesh = point_cloud_octree_mesh.into_inner();
        let render_octrees = render_octrees.into_inner();

        let Some(octree) = render_octrees.get(item.bin_key.asset_id) else {
            warn!("Missing octree when render");
            return RenderCommandResult::Skip;
        };

        let Some(node) = octree.nodes.get(&item.bin_key.node_id) else {
            warn!("Missing node when render");
            return RenderCommandResult::Skip;
        };

        pass.set_vertex_buffer(0, point_cloud_octree_mesh.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, node.data.points.slice(..));
        pass.set_index_buffer(
            point_cloud_octree_mesh.index_buffer.slice(..),
            0,
            IndexFormat::Uint32,
        );

        pass.draw_indexed(
            0..point_cloud_octree_mesh.index_count,
            0,
            0..node.data.num_points as u32,
        );

        RenderCommandResult::Success
    }
}

pub struct SetEmptyMeshBindGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetEmptyMeshBindGroup<I> {
    type Param = (SRes<RenderDevice>, SRes<MeshBindGroups>, SRes<MeshPipeline>);
    type ViewQuery = Has<MotionVectorPrepass>;
    type ItemQuery = ();

    #[inline]
    fn render<'w>(
        item: &P,
        has_motion_vector_prepass: bool,
        _item_query: Option<()>,
        (render_device, bind_groups, mesh_pipeline): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let bind_groups = bind_groups.into_inner();
        let mesh_pipeline = mesh_pipeline.into_inner();

        let entity = &item.main_entity();

        let Some(mesh_phase_bind_groups) = (match *bind_groups {
            MeshBindGroups::CpuPreprocessing(ref mesh_phase_bind_groups) => {
                Some(mesh_phase_bind_groups)
            }
            MeshBindGroups::GpuPreprocessing(ref mesh_phase_bind_groups) => {
                mesh_phase_bind_groups.get(&TypeId::of::<P>())
            }
        }) else {
            // This is harmless if e.g. we're rendering the `Shadow` phase and
            // there weren't any shadows.
            return RenderCommandResult::Success;
        };

        let Some(bind_group) =
            mesh_phase_bind_groups.get(AssetId::<Mesh>::default(), None, false, false, false)
        else {
            warn!("FAILURE");
            return RenderCommandResult::Failure(
                "The MeshBindGroups resource wasn't set in the render phase. \
                It should be set by the prepare_mesh_bind_group system.\n\
                This is a bevy bug! Please open an issue.",
            );
        };

        let mut dynamic_offsets: [u32; 5] = Default::default();

        info!("set bindgroup at index {}", I);
        pass.set_bind_group(I, bind_group, &dynamic_offsets[0..0]);

        RenderCommandResult::Success
    }
}
