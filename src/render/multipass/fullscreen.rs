use core::marker::PhantomData;

use bevy::{
    app::{App, Plugin},
    ecs::{
        component::Component,
        entity::Entity,
        error::BevyError,
        schedule::IntoScheduleConfigs,
        system::{Commands, Query, Res},
    },
    render::{
        render_resource::{
            binding_types::{texture_2d, texture_2d_multisampled},
            BindGroup, BindGroupEntry, CachedRenderPipelineId, IntoBinding, PipelineCache,
        },
        renderer::RenderDevice,
        view::Msaa,
        Render, RenderApp, RenderStartup, RenderSystems,
    },
};

use crate::render::{
    multipass::{
        init_fullscreen_material, prepare_fullscreen_material_pipelines,
        FullscreenMaterialPassPipeline, ViewMultipassTextures,
    },
    PointCloudMaterial, VisibleNodesTexture,
};

pub struct FullscreenPassPlugin<M, const PASS: usize> {
    _phantom: PhantomData<fn() -> M>,
}

impl<M: PointCloudMaterial, const PASS: usize> Default for FullscreenPassPlugin<M, PASS> {
    /// Creates a new [`PrepassPlugin`] with the given debug flags.
    fn default() -> Self {
        FullscreenPassPlugin {
            _phantom: PhantomData,
        }
    }
}

impl<M: PointCloudMaterial, const PASS: usize> Plugin for FullscreenPassPlugin<M, PASS> {
    fn build(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_systems(RenderStartup, init_fullscreen_material::<M, PASS>)
            .add_systems(
                Render,
                (
                    prepare_fullscreen_material_pipelines::<M, PASS>.in_set(RenderSystems::Prepare),
                    prepare_fullscreen_bind_groups::<M, PASS>
                        .in_set(RenderSystems::PrepareBindGroups),
                ),
            );
    }
}

#[derive(Component)]
pub struct FullscreenMaterialPipelineId<M: PointCloudMaterial, const PASS: usize> {
    pub pipeline_id: CachedRenderPipelineId,
    pub _phantom: PhantomData<M>,
}

/// Holds the bind groups for both main textures
///
/// We can't know ahead of time which one is the source or destination so we create a bind group
/// for both
#[derive(Component)]
pub struct FullscreenMaterialBindGroup<M: PointCloudMaterial, const PASS: usize> {
    pub bind_group: Option<BindGroup>,
    _marker: PhantomData<M>,
}

impl<M: PointCloudMaterial, const PASS: usize> Default for FullscreenMaterialBindGroup<M, PASS> {
    fn default() -> Self {
        Self {
            bind_group: Default::default(),
            _marker: Default::default(),
        }
    }
}

/// Prepare the bind groups for both main textures for all views that have a [`FullscreenMaterial`]
pub fn prepare_fullscreen_bind_groups<M: PointCloudMaterial, const PASS: usize>(
    mut commands: Commands,
    mut view: Query<(
        Entity,
        &VisibleNodesTexture,
        &ViewMultipassTextures<M>,
        &Msaa,
        Option<&mut FullscreenMaterialBindGroup<M, PASS>>,
    )>,
    point_cloud_pipeline: Res<FullscreenMaterialPassPipeline>,
    pipeline_cache: Res<PipelineCache>,
    render_device: Res<RenderDevice>,
) {
    let passes = M::passes();
    let pass = &passes[PASS];

    for (entity, visible_nodes_texture, view_multipass_textures, msaa, mut existing_bind_group) in
        &mut view
    {
        // TODO: cache this layout somewhere ? (eg in a resource or component)
        let mut pointcloud_layout = point_cloud_pipeline.point_cloud_layout.clone();

        for input in &pass.inputs {
            let entry = match msaa {
                Msaa::Off => texture_2d(input.texture_sample_type)
                    .build(input.binding_slot, input.visibility),
                _ => texture_2d_multisampled(input.texture_sample_type)
                    .build(input.binding_slot, input.visibility),
            };
            pointcloud_layout.entries.push(entry);
        }

        let layout = &pipeline_cache.get_bind_group_layout(&pointcloud_layout);

        // Fetch or create the `ViewPointCloudPassBindGroup` component
        let mut fullscreen_view_bind_group = match existing_bind_group {
            Some(ref mut existing_bind_group) => std::mem::take(&mut **existing_bind_group),
            None => FullscreenMaterialBindGroup::default(),
        };

        if visible_nodes_texture.has_changed || view_multipass_textures.has_changed {
            // first remove the previous bind group
            fullscreen_view_bind_group.bind_group = None;

            // then create a new if a texture is available
            if let Some(ref texture) = visible_nodes_texture.texture {
                let mut bind_group_entries = Vec::with_capacity(1 + pass.inputs.len());

                bind_group_entries.push(BindGroupEntry {
                    // keep the same binding index as in geometry passes
                    binding: 2,
                    resource: texture.default_view.into_binding(),
                });

                for input in &pass.inputs {
                    let pass_texture = view_multipass_textures
                        .get(&input.source_pass)
                        .ok_or_else(|| {
                            BevyError::from(format!(
                                "Pass {} references unexisting pass with name {}",
                                pass.name, input.source_pass
                            ))
                        })
                        .unwrap();
                    bind_group_entries.push(BindGroupEntry {
                        binding: input.binding_slot,
                        resource: pass_texture.texture.default_view.into_binding(),
                    });
                }

                let bind_group = render_device.create_bind_group(
                    "view_fullscreen_point_cloud_pass",
                    layout,
                    &bind_group_entries,
                );

                fullscreen_view_bind_group.bind_group = Some(bind_group);
            }

            let mut entity_commands = commands.entity(entity);
            entity_commands.insert(fullscreen_view_bind_group);
        }
    }
}
