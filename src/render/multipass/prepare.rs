use std::{any::type_name, marker::PhantomData};

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
            TextureDimension, TextureUsages,
        },
        renderer::RenderDevice,
        sync_world::MainEntity,
        texture::{ColorAttachment, TextureCache},
        view::{ExtractedView, Msaa},
    },
    shader::{Shader, ShaderRef},
};

use crate::render::{
    multipass::{
        FullscreenMaterialPassPipeline, FullscreenMaterialPipelineId,
        FullscreenMaterialPipelineSpecializer, FullscreenPassMaterialPipelineKey, MultipassTexture,
        Opaque3dMultipass, PreparedPointCloudFullscreenPass,
        SpecializedFullscreenPointCloudPipelines, ViewMultipassTextures,
        ViewPointCloudPassBindGroup,
    },
    PointCloud3d, PointCloudMaterial, PointCloudMaterialTargets, PointCloudPipeline,
    PreparedPointCloudUniforms, PreparedViewSettingsUniform, VisibleNodesTexture,
};

// Prepares the textures used by the prepass
pub fn prepare_multipass_textures<M: PointCloudMaterial>(
    material_targets: Res<PointCloudMaterialTargets<M>>,
    mut texture_cache: ResMut<TextureCache>,
    render_device: Res<RenderDevice>,
    // check only against first pass for simplification
    opaque_3d_multipass_phases: Res<ViewBinnedRenderPhases<Opaque3dMultipass<M, 0>>>,
    mut views_3d: Query<(
        &ExtractedCamera,
        &ExtractedView,
        &Msaa,
        &mut ViewMultipassTextures<M>,
    )>,
) {
    for (camera, view, msaa, mut view_multipass_textures) in &mut views_3d {
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

        for (name, target) in &material_targets.required_textures {
            let texture = view_multipass_textures
                .entry(name.clone())
                .or_insert_with(|| {
                    let descriptor = TextureDescriptor {
                        label: Some("multipass_texture"),
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
                    let texture = texture_cache.get(&render_device, descriptor);

                    MultipassTexture {
                        color_attachment: None,
                        texture,
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
        &VisibleNodesTexture,
        &PreparedViewSettingsUniform<M>,
        &ViewMultipassTextures<M>,
        &Msaa,
        Option<&mut ViewPointCloudPassBindGroup<M, PASS>>,
    )>,
    prepared_point_cloud_uniforms: Res<PreparedPointCloudUniforms>,
    items: Query<&MainEntity, With<PointCloud3d>>,
) {
    let passes = M::passes();
    let pass = &passes[PASS];

    for (
        entity,
        visible_nodes_texture,
        prepared_view_settings_uniform,
        view_multipass_textures,
        msaa,
        mut existing_view_point_cloud_bind_group,
    ) in &mut views
    {
        // TODO: cache this layout somewhere ? (eg in a resource or component)
        let mut pointcloud_layout = point_cloud_pipeline.point_cloud_layout.clone();
        pointcloud_layout
            .entries
            .push(uniform_buffer::<M::ViewSettings>(false).build(3, ShaderStages::VERTEX_FRAGMENT));

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
        let mut view_point_cloud_bind_groups = match existing_view_point_cloud_bind_group {
            Some(ref mut view_point_cloud_bind_groups) => {
                std::mem::take(&mut **view_point_cloud_bind_groups)
            }
            None => ViewPointCloudPassBindGroup::default(),
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

pub fn init_fullscreen_material<M: PointCloudMaterial, const PASS: usize>(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let pass = &M::passes()[PASS];

    let Some(fragment_shader) = get_shader(&asset_server, &pass.fragment_shader) else {
        panic!(
            "No fragment shader defined for point cloud material {} pass {}",
            type_name::<M>(),
            PASS,
        );
    };

    commands.insert_resource(PreparedPointCloudFullscreenPass::<M, PASS> {
        fragment_shader,
        vertex_shader: get_shader(&asset_server, &pass.vertex_shader),
        _phantom: PhantomData,
    });
}

pub fn prepare_fullscreen_material_pipelines<M: PointCloudMaterial, const PASS: usize>(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    views: Query<(Entity, &ExtractedView), With<ExtractedCamera>>,
    view_key_cache: Res<ViewKeyCache>,
    fullscreen_pipeline: Res<FullscreenMaterialPassPipeline>,
    mut specialized_fullscreen_point_cloud_pipelines: ResMut<
        SpecializedFullscreenPointCloudPipelines<FullscreenMaterialPipelineSpecializer>,
    >,
    prepared_point_cloud_fullscreen_pass: Res<PreparedPointCloudFullscreenPass<M, PASS>>,
) {
    let pass = &M::passes()[PASS];
    for (entity, view) in &views {
        let Some(&view_key) = view_key_cache.get(&view.retained_view_entity) else {
            continue;
        };

        let pipeline_key = FullscreenPassMaterialPipelineKey {
            view_key,
            pass: PASS,
        };

        let pipeline_specializer = FullscreenMaterialPipelineSpecializer {
            vertex_shader: prepared_point_cloud_fullscreen_pass.vertex_shader.clone(),
            fragment_shader: prepared_point_cloud_fullscreen_pass.fragment_shader.clone(),
            fullscreen_pipeline: fullscreen_pipeline.clone(),
            pass,
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
