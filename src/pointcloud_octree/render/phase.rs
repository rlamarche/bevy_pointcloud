use crate::octree::extract::RenderOctrees;
use crate::octree::storage::NodeId;
use crate::point_cloud_material::PointCloudMaterial3d;
use crate::pointcloud_octree::asset::PointCloudOctree;
use crate::pointcloud_octree::component::PointCloudOctree3d;
use crate::pointcloud_octree::extract::RenderPointCloudNodeData;
use crate::pointcloud_octree::render::data::PreparedPointCloudOctree3dUniform;
use crate::pointcloud_octree::render::prepare::VisibleNodesTextureBindGroup;
use crate::render::material::RenderPointCloudMaterial;
use bevy_asset::AssetId;
use bevy_core_pipeline::oit::OrderIndependentTransparencySettingsOffset;
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use bevy_log::warn;
use bevy_pbr::{
    MeshViewBindGroup, ViewEnvironmentMapUniformOffset, ViewFogUniformOffset,
    ViewLightProbesUniformOffset, ViewLightsUniformOffset, ViewScreenSpaceReflectionsUniformOffset,
};
use bevy_platform::collections::hash_map::Entry;
use bevy_platform::collections::HashMap;
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_phase::{
    BinnedPhaseItem, DrawError, DrawFunctions, PhaseItem, TrackedRenderPass,
};
use bevy_render::view::{RetainedViewEntity, ViewUniformOffset};
use smallvec::{smallvec, SmallVec};

pub trait PointCloudOctree3dPhase {
    fn node_id(&self) -> NodeId;
}

/// Data that must be identical in order to *batch* phase items together.
///
/// Note that a *batch set* (if multi-draw is in use) contains multiple batches.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PointCloudOctree3dBinKey {
    /// The asset that this phase item is associated with.
    ///
    /// Normally, this is the ID of the mesh, but for non-mesh items it might be
    /// the ID of another type of asset.
    pub asset_id: AssetId<PointCloudOctree>,
    pub node_id: NodeId,
}

#[derive(Resource, Deref, DerefMut)]
pub struct ViewOctreeNodesRenderPhases<BPI>(
    pub HashMap<RetainedViewEntity, OctreeNodeRenderPhase<BPI>>,
)
where
    BPI: PhaseItem;

impl<BPI: PhaseItem> Default for ViewOctreeNodesRenderPhases<BPI> {
    fn default() -> Self {
        Self(HashMap::default())
    }
}

impl<BPI> ViewOctreeNodesRenderPhases<BPI>
where
    BPI: BinnedPhaseItem + PointCloudOctree3dPhase,
{
    pub fn prepare_for_new_frame(&mut self, retained_view_entity: RetainedViewEntity) {
        match self.entry(retained_view_entity) {
            Entry::Occupied(mut entry) => entry.get_mut().prepare_for_new_frame(),
            Entry::Vacant(entry) => {
                entry.insert(OctreeNodeRenderPhase::<BPI>::new());
            }
        }
    }
}

pub struct OctreeNodeRenderPhase<BPI>
where
    BPI: PhaseItem,
{
    pub phases: Vec<BPI>,
}

impl<BPI> OctreeNodeRenderPhase<BPI>
where
    BPI: BinnedPhaseItem + PointCloudOctree3dPhase,
{
    fn new() -> Self {
        Self {
            phases: Vec::default(),
        }
    }

    pub fn prepare_for_new_frame(&mut self) {
        self.phases.clear();
    }

    pub fn render<'w>(
        &self,
        render_pass: &mut TrackedRenderPass<'w>,
        world: &'w World,
        view: Entity,
    ) -> Result<(), DrawError> {
        {
            let draw_functions = world.resource::<DrawFunctions<BPI>>();
            let mut draw_functions = draw_functions.write();
            draw_functions.prepare(world);
            // Make sure to drop the reader-writer lock here to avoid recursive
            // locks.
        }

        self.bind_view_data(render_pass, world, view)?;

        {
            let draw_functions = world.resource::<DrawFunctions<BPI>>();
            let mut draw_functions = draw_functions.write();

            for phase_item in &self.phases {
                let Some(draw_function) = draw_functions.get_mut(phase_item.draw_function()) else {
                    continue;
                };

                draw_function.draw(world, render_pass, view, &phase_item)?;
            }
        }

        Ok(())
    }

    /// Binds octree node as in draw function [`crate::pointcloud_octree::render::draw::SetPointCloudOctreeNodeUniformGroup`]
    #[allow(unused)]
    fn bind_octree_node<'w>(
        &self,
        render_pass: &mut TrackedRenderPass<'w>,
        world: &'w World,
        item: Entity,
        node_id: NodeId,
    ) -> Result<(), DrawError> {
        let render_octrees = world.resource::<RenderOctrees<RenderPointCloudNodeData>>();

        let Some(point_cloud_octree_3d) =
            world.entity(item).get_components::<&PointCloudOctree3d>()
        else {
            warn!("Unable to get item components");
            return Ok(());
        };

        let Some(octree) = render_octrees.get(point_cloud_octree_3d) else {
            warn!("Missing octree when render");
            return Ok(());
        };

        let Some(node) = octree.nodes.get(&node_id) else {
            warn!("Missing node when render");
            return Ok(());
        };

        render_pass.set_bind_group(3, &node.data.uniform, &[]);

        Ok(())
    }

    /// Binds item data as in draw function [`crate::pointcloud_octree::render::data::SetPointCloudOctree3dUniformGroup`]
    /// and [`crate::render::material::SetPointCloudMaterialGroup`]
    #[allow(unused)]
    fn bind_item_data<'w>(
        &self,
        render_pass: &mut TrackedRenderPass<'w>,
        world: &'w World,
        item: Entity,
    ) -> Result<(), DrawError> {
        let render_point_cloud_materials =
            world.resource::<RenderAssets<RenderPointCloudMaterial>>();

        let Some((prepared_point_cloud_octree_3d_uniform, point_cloud_material_3d)) = world
            .entity(item)
            .get_components::<(&PreparedPointCloudOctree3dUniform, &PointCloudMaterial3d)>()
        else {
            warn!("Unable to get item components");
            return Ok(());
        };

        render_pass.set_bind_group(
            1,
            &prepared_point_cloud_octree_3d_uniform.prepared.bind_group,
            &[],
        );

        let Some(render_point_cloud_material) =
            render_point_cloud_materials.get(point_cloud_material_3d)
        else {
            warn!("Unable to get item material");
            return Ok(());
        };

        render_pass.set_bind_group(2, &render_point_cloud_material.uniform, &[]);

        Ok(())
    }

    fn bind_view_data<'w>(
        &self,
        render_pass: &mut TrackedRenderPass<'w>,
        world: &'w World,
        view: Entity,
    ) -> Result<(), DrawError> {
        let Some((
            view_uniform,
            view_lights,
            view_fog,
            view_light_probes,
            view_ssr,
            view_environment_map,
            mesh_view_bind_group,
            maybe_oit_layers_count_offset,
            visible_nodes_texture,
        )) = world.entity(view).get_components::<(
            &ViewUniformOffset,
            &ViewLightsUniformOffset,
            &ViewFogUniformOffset,
            &ViewLightProbesUniformOffset,
            &ViewScreenSpaceReflectionsUniformOffset,
            &ViewEnvironmentMapUniformOffset,
            &MeshViewBindGroup,
            Option<&OrderIndependentTransparencySettingsOffset>,
            &VisibleNodesTextureBindGroup,
        )>()
        else {
            warn!("Unable to get view components");
            return Ok(());
        };

        let mut offsets: SmallVec<[u32; 8]> = smallvec![
            view_uniform.offset,
            view_lights.offset,
            view_fog.offset,
            **view_light_probes,
            **view_ssr,
            **view_environment_map,
        ];
        if let Some(layers_count_offset) = maybe_oit_layers_count_offset {
            offsets.push(layers_count_offset.offset);
        }
        render_pass.set_bind_group(0, &mesh_view_bind_group.main, &offsets);
        render_pass.set_bind_group(4, &visible_nodes_texture.texture, &[]);

        Ok(())
    }
}
