use bevy::{
    color::LinearRgba,
    ecs::{
        entity::Entity,
        error::BevyError,
        query::With,
        system::{Commands, Query, Res, ResMut},
    },
    image::ToExtents,
    render::{
        camera::ExtractedCamera,
        render_phase::ViewBinnedRenderPhases,
        render_resource::{
            binding_types::{texture_2d, texture_2d_multisampled},
            BindGroupEntry, IntoBinding, PipelineCache, TextureDescriptor, TextureDimension,
            TextureUsages,
        },
        renderer::RenderDevice,
        sync_world::MainEntity,
        texture::{ColorAttachment, TextureCache},
        view::{ExtractedView, Msaa},
    },
};

use crate::{
    multipass::{
        MultipassTexture, Opaque3dMultipass, ViewMultipassTextures, ViewPointCloudPassBindGroup,
    },
    Material, PointCloud3d, PointCloudMaterialTargets, PointCloudPipeline,
    PreparedPointCloudUniforms, VisibleNodesTexture,
};

// Prepares the textures used by the prepass
pub fn prepare_multipass_textures<M: Material>(
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
            view_multipass_textures
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
                        color_attachment: ColorAttachment::new(
                            texture.clone(),
                            None,
                            None,
                            Some(LinearRgba::BLACK.into()),
                        ),
                        texture,
                    }
                });
        }
    }
}

/// Same as [`crate::prepare_visible_nodes_texture_bind_groups`] but at material/pass level.
pub fn prepare_pointcloud_bind_groups<M: Material, const PASS: usize>(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    point_cloud_pipeline: Res<PointCloudPipeline>,
    render_device: Res<RenderDevice>,
    views: Query<(
        Entity,
        &VisibleNodesTexture,
        &ViewMultipassTextures<M>,
        &Msaa,
    )>,
    mut existing_view_point_cloud_bind_groups: Query<&mut ViewPointCloudPassBindGroup<M, PASS>>,
    prepared_point_cloud_uniforms: Res<PreparedPointCloudUniforms>,
    items: Query<&MainEntity, With<PointCloud3d>>,
) {
    let passes = M::passes();
    let pass = &passes[PASS];

    for (entity, visible_nodes_texture, view_multipass_textures, msaa) in &views {
        // TODO: cache this layout somewhere ? (eg in a resource)
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
        let mut view_point_cloud_bind_groups =
            match existing_view_point_cloud_bind_groups.get_mut(entity) {
                Ok(ref mut view_point_cloud_bind_groups) => {
                    std::mem::take(&mut **view_point_cloud_bind_groups)
                }
                Err(_) => ViewPointCloudPassBindGroup::default(),
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
