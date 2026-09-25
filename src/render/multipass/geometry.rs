use core::marker::PhantomData;

use bevy::{
    app::{App, Plugin},
    ecs::schedule::IntoScheduleConfigs,
    pbr::{collect_meshes_for_gpu_building, set_mesh_motion_vector_flags, MeshPipeline},
    render::{
        mesh::RenderMesh,
        render_asset::prepare_assets,
        render_phase::{
            AddRenderCommand, BinnedRenderPhasePlugin, DrawFunctions, ViewBinnedRenderPhases,
        },
        ExtractSchedule, Render, RenderApp, RenderDebugFlags, RenderSystems,
    },
};

use crate::render::{
    multipass::{
        extract_camera_multipass_phase, prepare_pointcloud_bind_groups,
        queue_multipass_material_meshes, specialize_multipass_material_meshes, DrawMultipass,
        Opaque3dMultipass,
    },
    PointCloudMaterial,
};

pub struct GeometryPassPlugin<M, const PASS: usize> {
    pub debug_flags: RenderDebugFlags,
    _phantom: PhantomData<fn() -> M>,
}

impl<M: PointCloudMaterial, const PASS: usize> GeometryPassPlugin<M, PASS> {
    /// Creates a new [`PrepassPlugin`] with the given debug flags.
    pub fn new(debug_flags: RenderDebugFlags) -> Self {
        GeometryPassPlugin {
            debug_flags,
            _phantom: PhantomData,
        }
    }
}

impl<M: PointCloudMaterial, const PASS: usize> Plugin for GeometryPassPlugin<M, PASS> {
    fn build(&self, app: &mut App) {
        app.add_plugins(BinnedRenderPhasePlugin::<
            Opaque3dMultipass<M, PASS>,
            MeshPipeline,
        >::new(self.debug_flags));

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<DrawFunctions<Opaque3dMultipass<M, PASS>>>()
            // TODO: is this needed given we load `BinnedRenderPhasePlugin` above ?
            .init_resource::<ViewBinnedRenderPhases<Opaque3dMultipass<M, PASS>>>();

        render_app
            // .init_gpu_resource::<PendingMultipassMeshMaterialQueues>()
            .add_render_command::<Opaque3dMultipass<M, PASS>, DrawMultipass<M, PASS>>()
            .add_systems(ExtractSchedule, extract_camera_multipass_phase::<M, PASS>)
            .add_systems(
                Render,
                (
                    // check_multipass_views_need_specialization.in_set(PrepareAssets),
                    specialize_multipass_material_meshes::<M, PASS>
                        .in_set(RenderSystems::PrepareMeshes)
                        .after(prepare_assets::<RenderMesh>)
                        .after(collect_meshes_for_gpu_building)
                        .after(set_mesh_motion_vector_flags),
                    prepare_pointcloud_bind_groups::<M, PASS>
                        .in_set(RenderSystems::PrepareBindGroups),
                    queue_multipass_material_meshes::<M, PASS>.in_set(RenderSystems::QueueMeshes),
                ),
            );
    }
}
