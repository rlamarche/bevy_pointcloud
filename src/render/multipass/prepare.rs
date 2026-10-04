use std::{
    any::{type_name, TypeId},
    marker::PhantomData,
};

use bevy::{
    asset::{AssetServer, Handle},
    color::LinearRgba,
    ecs::{
        entity::Entity,
        error::BevyError,
        query::With,
        system::{Commands, Query, Res, ResMut},
    },
    image::ToExtents,
    log::error,
    pbr::ViewKeyCache,
    render::{
        camera::ExtractedCamera,
        render_phase::ViewBinnedRenderPhases,
        render_resource::{
            binding_types::{texture_2d, texture_2d_multisampled, uniform_buffer},
            BindGroupEntry, IntoBinding, PipelineCache, ShaderStages, TextureDescriptor,
            TextureDimension, TextureUsages, TextureViewDescriptor,
        },
        renderer::RenderDevice,
        sync_world::MainEntity,
        texture::{CachedTexture, ColorAttachment},
        view::{ExtractedView, Msaa},
    },
    shader::{Shader, ShaderRef},
};

use crate::{
    render::{
        multipass::{
            FullscreenMaterialPassPipeline, FullscreenMaterialPipelineId,
            FullscreenMaterialPipelineSpecializer, FullscreenPassMaterialPipelineKey,
            MultipassTexture, Opaque3dMultipass, SpecializedFullscreenPointCloudPipelines,
            ViewMultipassTextures, ViewPointCloudPassBindGroups,
        },
        ErasedMaterialPreparedPasses, ErasedViewSettingsKey, MaterialFragmentShader,
        MaterialVertexShader, PassOutput, PointCloud3d, PointCloudMaterial, PointCloudPipeline,
        PreparedPointCloudUniforms, PreparedViewSettingsUniform, VisibleNodesTexture,
    },
    ViewSettings,
};

// Prepares the textures used by the prepass
pub fn prepare_multipass_textures<M: PointCloudMaterial>(
    render_device: Res<RenderDevice>,
    // check only against first pass for simplification
    opaque_3d_multipass_phases: Res<ViewBinnedRenderPhases<Opaque3dMultipass<M, 0>>>,
    mut views_3d: Query<(
        &ExtractedCamera,
        &ExtractedView,
        &Msaa,
        &mut ViewMultipassTextures<M>,
        &ErasedMaterialPreparedPasses,
    )>,
) {
    let type_id = TypeId::of::<M>();

    for (camera, view, msaa, mut view_multipass_textures, material_prepared_passes) in &mut views_3d
    {
        let Some(prepared_passes) = material_prepared_passes.get(&type_id) else {
            continue;
        };

        if !opaque_3d_multipass_phases.contains_key(&view.retained_view_entity) {
            view_multipass_textures.textures.clear();
            view_multipass_textures.has_changed = true;
            view_multipass_textures.size = Default::default();
            continue;
        };

        let Some(physical_target_size) = camera.physical_target_size else {
            continue;
        };

        let size = physical_target_size.to_extents();

        // if the size has changed, clear textures and store new size
        if !size.eq(&view_multipass_textures.size) {
            view_multipass_textures.size = size;
            view_multipass_textures.has_changed = true;
            view_multipass_textures.clear();
        } else {
            view_multipass_textures.has_changed = false;
        }

        for (&id, pass) in prepared_passes.iter() {
            let PassOutput::TransientTarget(target) = &pass.output else {
                continue;
            };

            let texture = view_multipass_textures.entry(id).or_insert_with(|| {
                let texture_label =
                    format!("multipass_texture_{}_{}", type_name::<M>(), pass.label);
                let descriptor = TextureDescriptor {
                    label: Some(&texture_label),
                    size,
                    mip_level_count: 1,
                    sample_count: msaa.samples(),
                    dimension: TextureDimension::D2,
                    format: target.format,
                    usage: TextureUsages::COPY_DST
                        | TextureUsages::RENDER_ATTACHMENT
                        | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                };
                let texture = render_device.create_texture(&descriptor);
                let default_view = texture.create_view(&TextureViewDescriptor::default());

                MultipassTexture {
                    color_attachment: None,
                    texture: CachedTexture {
                        texture,
                        default_view,
                    },
                }
            });

            // recreate the attachment to make sure it's cleared every frame
            texture.color_attachment = Some(ColorAttachment::new(
                texture.texture.clone(),
                None,
                None,
                Some(LinearRgba::NONE.into()),
            ));
        }
    }
}

/// Same as [`crate::prepare_visible_nodes_texture_bind_groups`] but at material/pass level.
pub fn prepare_pointcloud_bind_groups<M: PointCloudMaterial, const PASS: usize>(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    point_cloud_pipeline: Res<PointCloudPipeline>,
    render_device: Res<RenderDevice>,
    mut views: Query<(
        Entity,
        &ErasedMaterialPreparedPasses,
        &VisibleNodesTexture,
        &PreparedViewSettingsUniform<M>,
        &ViewMultipassTextures<M>,
        &Msaa,
        Option<&mut ViewPointCloudPassBindGroups<M, PASS>>,
    )>,
    prepared_point_cloud_uniforms: Res<PreparedPointCloudUniforms>,
    items: Query<&MainEntity, With<PointCloud3d>>,
) {
    let type_id = TypeId::of::<M>();

    for (
        entity,
        material_prepared_passes,
        visible_nodes_texture,
        prepared_view_settings_uniform,
        view_multipass_textures,
        msaa,
        mut existing_view_point_cloud_bind_groups,
    ) in &mut views
    {
        let Some(prepared_passes) = material_prepared_passes.get(&type_id) else {
            if existing_view_point_cloud_bind_groups.is_some() {
                // cleanup bind group because this pass does not exists anymore
                // TODO: do it in `prepare_material_view_settings_key_and_passes` by making it
                // erased (like [`ViewPointCloudBindGroups`]).
                commands
                    .entity(entity)
                    .remove::<ViewPointCloudPassBindGroups<M, PASS>>();
            }
            continue;
        };
        let Some(pass) = prepared_passes.get(&PASS) else {
            if existing_view_point_cloud_bind_groups.is_some() {
                // cleanup bind group because this pass does not exists anymore
                commands
                    .entity(entity)
                    .remove::<ViewPointCloudPassBindGroups<M, PASS>>();
            }
            continue;
        };

        // TODO: cache this layout somewhere ? (eg in a resource or component)
        let mut pointcloud_layout = point_cloud_pipeline.point_cloud_layout.clone();
        pointcloud_layout.entries.push(
            uniform_buffer::<<M::ViewSettings as ViewSettings>::Data>(false)
                .build(3, ShaderStages::VERTEX_FRAGMENT),
        );

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
        let mut view_point_cloud_bind_groups = match existing_view_point_cloud_bind_groups {
            Some(ref mut view_point_cloud_bind_groups) => {
                std::mem::take(&mut **view_point_cloud_bind_groups)
            }
            None => ViewPointCloudPassBindGroups::default(),
        };

        for main_entity in items {
            let Some(prepared_uniform) = prepared_point_cloud_uniforms.get(main_entity) else {
                // free unused bind groups
                view_point_cloud_bind_groups.bind_groups.remove(main_entity);

                continue;
            };

            if visible_nodes_texture.has_changed || view_multipass_textures.has_changed {
                // first remove the previous bind group
                view_point_cloud_bind_groups.bind_groups.remove(main_entity);

                // then create a new if a texture is available
                if let Some(ref texture) = visible_nodes_texture.texture {
                    let mut bind_group_entries = Vec::with_capacity(3 + pass.inputs.len());

                    bind_group_entries.push(BindGroupEntry {
                        binding: 0,
                        resource: prepared_uniform.mesh_uniform_buffer.binding().unwrap(),
                    });
                    bind_group_entries.push(BindGroupEntry {
                        binding: 1,
                        resource: prepared_uniform.point_cloud_uniform_buffer.into_binding(),
                    });
                    bind_group_entries.push(BindGroupEntry {
                        binding: 2,
                        resource: texture.default_view.into_binding(),
                    });
                    bind_group_entries.push(BindGroupEntry {
                        binding: 3,
                        resource: prepared_view_settings_uniform
                            .view_settings_uniform_buffer
                            .into_binding(),
                    });

                    for input in &pass.inputs {
                        let pass_texture = view_multipass_textures
                            .get(&input.source_pass)
                            .ok_or_else(|| {
                                BevyError::from(format!(
                                    "Pass {} references unexisting pass with index {}",
                                    pass.label, input.source_pass
                                ))
                            })
                            .unwrap();
                        bind_group_entries.push(BindGroupEntry {
                            binding: input.binding_slot,
                            resource: pass_texture.texture.default_view.into_binding(),
                        });
                    }

                    let bind_group = render_device.create_bind_group(
                        "view_point_cloud_pass",
                        layout,
                        &bind_group_entries,
                    );

                    view_point_cloud_bind_groups
                        .bind_groups
                        .insert(*main_entity, bind_group);
                }
            }
        }

        let mut entity_commands = commands.entity(entity);
        entity_commands.insert(view_point_cloud_bind_groups);
    }
}

pub fn get_shader(asset_server: &AssetServer, shader_ref: &ShaderRef) -> Option<Handle<Shader>> {
    match shader_ref {
        ShaderRef::Default => None,
        ShaderRef::Handle(handle) => Some(handle.clone()),
        ShaderRef::Path(path) => Some(asset_server.load(path)),
    }
}

pub fn prepare_fullscreen_material_pipelines<M: PointCloudMaterial, const PASS: usize>(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    views: Query<
        (
            Entity,
            &ExtractedView,
            &M::ViewSettings,
            &ErasedMaterialPreparedPasses,
            Option<&FullscreenMaterialPipelineId<M, PASS>>,
        ),
        With<ExtractedCamera>,
    >,
    view_key_cache: Res<ViewKeyCache>,
    fullscreen_pipeline: Res<FullscreenMaterialPassPipeline>,
    mut specialized_fullscreen_point_cloud_pipelines: ResMut<
        SpecializedFullscreenPointCloudPipelines<FullscreenMaterialPipelineSpecializer>,
    >,
) {
    let type_id = TypeId::of::<M>();
    for (
        entity,
        view,
        view_settings,
        material_prepared_passes,
        maybe_fullscreen_material_pipeline_id,
    ) in &views
    {
        let Some(prepared_passes) = material_prepared_passes.get(&type_id) else {
            if maybe_fullscreen_material_pipeline_id.is_some() {
                // remove previous pipeline id
                commands
                    .entity(entity)
                    .remove::<FullscreenMaterialPipelineId<M, PASS>>();
            }
            continue;
        };

        let Some(pass) = prepared_passes.get(&PASS) else {
            if maybe_fullscreen_material_pipeline_id.is_some() {
                // remove previous pipeline id
                commands
                    .entity(entity)
                    .remove::<FullscreenMaterialPipelineId<M, PASS>>();
            }
            continue;
        };

        let Some(&view_key) = view_key_cache.get(&view.retained_view_entity) else {
            continue;
        };
        let pipeline_key = FullscreenPassMaterialPipelineKey {
            view_key,
            pass: PASS,
            settings_key: ErasedViewSettingsKey::new(view_settings.pipeline_key()),
        };

        let pipeline_specializer = FullscreenMaterialPipelineSpecializer {
            vertex_shader: pass.get_shader(MaterialVertexShader),
            fragment_shader: pass.get_shader(MaterialFragmentShader),
            fullscreen_pipeline: fullscreen_pipeline.clone(),
            pass: pass.clone(),
        };

        match specialized_fullscreen_point_cloud_pipelines.specialize(
            &pipeline_cache,
            &pipeline_specializer,
            pipeline_key,
        ) {
            Ok(pipeline_id) => {
                commands
                    .entity(entity)
                    .insert(FullscreenMaterialPipelineId::<M, PASS> {
                        pipeline_id,
                        _phantom: PhantomData,
                    });
            }
            Err(err) => error!("{}", err),
        }
    }
}
