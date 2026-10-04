use std::{any::TypeId, hash::Hash, sync::Arc};

use bevy::{
    asset::{AssetId, AssetServer, Handle},
    core_pipeline::{
        core_3d::{AlphaMask3d, Opaque3d, Transparent3d},
        deferred::{AlphaMask3dDeferred, Opaque3dDeferred},
        prepass::{AlphaMask3dPrepass, Opaque3dPrepass},
    },
    ecs::{
        component::Component,
        entity::Entity,
        query::{Changed, With},
        system::{
            lifetimeless::{SRes, SResMut},
            Commands, Query, Res, ResMut, SystemParamItem,
        },
    },
    material::{
        descriptor::BindGroupLayoutDescriptor,
        key::{ErasedMaterialKey, ErasedMeshPipelineKey},
        labels::{
            DrawFunctionId, DrawFunctionLabel, InternedDrawFunctionLabel, InternedShaderLabel,
            ShaderLabel,
        },
        AlphaMode, OpaqueRendererMethod, RenderPhaseType,
    },
    pbr::{
        DirectionalLightViewEntities, MaterialBindGroupAllocators, MeshPipelineKey, Shadow,
        Transmissive3d,
    },
    platform::{
        collections::{hash_map::Entry, HashMap},
        hash::NoOpHash,
    },
    prelude::Deref,
    render::{
        erased_render_asset::{ErasedRenderAsset, PrepareAssetError},
        render_phase::DrawFunctions,
        render_resource::{
            binding_types::uniform_buffer, AsBindGroupError, BindGroupEntries,
            BindGroupLayoutEntry, PipelineCache, ShaderStages,
        },
        renderer::{RenderDevice, RenderQueue},
        sync_world::MainEntity,
        view::ExtractedView,
    },
    shader::{Shader, ShaderRef},
};
use derive_more::DerefMut;
use smallvec::SmallVec;

use crate::{
    render::{
        base_specialize,
        material::{clone_shader_ref, prepass_specialize, user_specialize},
        material_uses_bindless_resources,
        multipass::ViewMultipassTextures,
        prepass::{DrawDepthOnlyPrepass, DrawPrepass},
        BaseSpecializeFn, DeferredAlphaMaskDrawFunction, DeferredFragmentShader,
        DeferredOpaqueDrawFunction, DeferredVertexShader, DrawPointCloudMaterial,
        ErasedViewSettingsKey, MainPassAlphaMaskDrawFunction, MainPassOpaqueDrawFunction,
        MainPassTransmissiveDrawFunction, MainPassTransparentDrawFunction, MaterialFragmentShader,
        MaterialVertexShader, PassKind, PassProperties, PointCloudDirtySpecializations,
        PointCloudMaterial, PointCloudPipeline, PreparedPointCloudMaterial,
        PreparedPointCloudUniforms, PreparedViewSettingsUniform, PrepassAlphaMaskDrawFunction,
        PrepassFragmentShader, PrepassOpaqueDepthOnlyDrawFunction, PrepassOpaqueDrawFunction,
        PrepassSpecializeFn, PrepassVertexShader, RenderMaterialBindings,
        ShadowsDepthOnlyDrawFunction, ShadowsDrawFunction, UserSpecializeFn,
        ViewPointCloudBindGroups, VisibleNodesTexture,
    },
    PointCloud3d, PointCloudMaterial3d, ViewSettings,
};

/// Common material properties, calculated for a specific material instance.
#[derive(Default)]
pub struct PointCloudMaterialProperties {
    /// Is this material should be rendered by the deferred renderer when.
    /// [`AlphaMode::Opaque`] or [`AlphaMode::Mask`]
    pub render_method: OpaqueRendererMethod,
    /// The [`AlphaMode`] of this material.
    pub alpha_mode: AlphaMode,
    /// The bits in the [`ErasedMeshPipelineKey`] for this material.
    ///
    /// These are precalculated so that we can just "or" them together in
    /// [`queue_material_meshes`](https://docs.rs/bevy/latest/bevy/pbr/fn.queue_material_meshes.html).
    pub mesh_pipeline_key_bits: ErasedMeshPipelineKey,
    /// Add a bias to the view depth of the mesh which can be used to force a specific render order
    /// for meshes with equal depth, to avoid z-fighting.
    /// The bias is in depth-texture units so large values may be needed to overcome small depth
    /// differences.
    pub depth_bias: f32,
    /// Whether the material would like to read from
    /// [`ViewTransmissionTexture`](https://docs.rs/bevy/latest/bevy/core_pipeline/core_3d/struct.ViewTransmissionTexture.html).
    ///
    /// This allows taking color output from the [`Opaque3d`](https://docs.rs/bevy/latest/bevy/core_pipeline/core_3d/struct.Opaque3d.html)
    /// pass as an input, (for screen-space transmission) but requires rendering to take place in a
    /// separate [`Transmissive3d`](https://docs.rs/bevy/latest/bevy/core_pipeline/core_3d/struct.Transmissive3d.html) pass.
    pub reads_view_transmission_texture: bool,
    pub render_phase_type: RenderPhaseType,
    pub material_layout: Option<BindGroupLayoutDescriptor>,
    /// Backing array is a size of 4 because the [`StandardMaterial`](https://docs.rs/bevy/latest/bevy/pbr/struct.StandardMaterial.html)
    /// needs 4 draw functions by default
    pub draw_functions: SmallVec<[(InternedDrawFunctionLabel, DrawFunctionId); 4]>,
    /// Backing array is a size of 3 because the [`StandardMaterial`](https://docs.rs/bevy/latest/bevy/pbr/struct.StandardMaterial.html)
    /// has 3 custom shaders (`frag`, `prepass_frag`, `deferred_frag`) which is the
    /// most common use case
    pub shaders: SmallVec<[(InternedShaderLabel, Handle<Shader>); 3]>,
    /// Whether this material *actually* uses bindless resources, taking the
    /// platform support (or lack thereof) of bindless resources into account.
    pub bindless: bool,
    pub base_specialize: Option<BaseSpecializeFn>,
    pub prepass_specialize: Option<PrepassSpecializeFn>,
    pub user_specialize: Option<UserSpecializeFn>,
    /// The key for this material, typically a bitfield of flags that are used to modify
    /// the pipeline descriptor used for this material.
    pub material_key: ErasedMaterialKey,

    /// Whether shadows are enabled for this material
    pub shadows_enabled: bool,
    /// Whether prepass is enabled for this material
    pub prepass_enabled: bool,

    pub view_settings_layout_entry: Option<BindGroupLayoutEntry>,
}

impl PointCloudMaterialProperties {
    pub fn get_shader(&self, label: impl ShaderLabel) -> Option<Handle<Shader>> {
        self.shaders
            .iter()
            .find(|(inner_label, _)| inner_label == &label.intern())
            .map(|(_, shader)| shader)
            .cloned()
    }

    pub fn add_shader(&mut self, label: impl ShaderLabel, shader: Handle<Shader>) {
        self.shaders.push((label.intern(), shader));
    }

    pub fn get_draw_function(&self, label: impl DrawFunctionLabel) -> Option<DrawFunctionId> {
        self.draw_functions
            .iter()
            .find(|(inner_label, _)| inner_label == &label.intern())
            .map(|(_, shader)| shader)
            .cloned()
    }

    pub fn add_draw_function(
        &mut self,
        label: impl DrawFunctionLabel,
        draw_function: DrawFunctionId,
    ) {
        self.draw_functions.push((label.intern(), draw_function));
    }
}

impl<M: PointCloudMaterial> ErasedRenderAsset for PointCloudMaterial3d<M>
where
    M::Data: PartialEq + Eq + Hash + Clone,
{
    type SourceAsset = M;
    type ErasedAsset = PreparedPointCloudMaterial;

    type Param = (
        SRes<RenderDevice>,
        SRes<PipelineCache>,
        // SRes<DefaultOpaqueRendererMethod>,
        SResMut<MaterialBindGroupAllocators>,
        SResMut<RenderMaterialBindings>,
        (
            SRes<DrawFunctions<Opaque3d>>,
            SRes<DrawFunctions<AlphaMask3d>>,
            SRes<DrawFunctions<Transmissive3d>>,
            SRes<DrawFunctions<Transparent3d>>,
            // SRes<DrawFunctions<Opaque3dMultipass<M>>>,
            SRes<DrawFunctions<Opaque3dPrepass>>,
            SRes<DrawFunctions<AlphaMask3dPrepass>>,
            SRes<DrawFunctions<Opaque3dDeferred>>,
            SRes<DrawFunctions<AlphaMask3dDeferred>>,
            SRes<DrawFunctions<Shadow>>,
        ),
        SRes<AssetServer>,
        M::Param,
    );

    fn prepare_asset(
        material: Self::SourceAsset,
        material_id: AssetId<Self::SourceAsset>,
        (
            render_device,
            pipeline_cache,
            // default_opaque_render_method,
            bind_group_allocators,
            render_material_bindings,
            (
                opaque_draw_functions,
                alpha_mask_draw_functions,
                transmissive_draw_functions,
                transparent_draw_functions,
                // multipass_draw_functions,
                opaque_prepass_draw_functions,
                alpha_mask_prepass_draw_functions,
                opaque_deferred_draw_functions,
                alpha_mask_deferred_draw_functions,
                shadow_draw_functions,
            ),
            asset_server,
            material_param,
        ): &mut SystemParamItem<Self::Param>,
    ) -> Result<Self::ErasedAsset, PrepareAssetError<Self::SourceAsset>> {
        let material_layout = M::bind_group_layout_descriptor(render_device);
        let actual_material_layout = pipeline_cache.get_bind_group_layout(&material_layout);

        let binding = match material.unprepared_bind_group(
            &actual_material_layout,
            render_device,
            material_param,
            false,
        ) {
            Ok(unprepared) => {
                let bind_group_allocator =
                    bind_group_allocators.get_mut(&TypeId::of::<M>()).unwrap();
                // Allocate or update the material.
                match render_material_bindings.entry(material_id.into()) {
                    Entry::Occupied(mut occupied_entry) => {
                        // TODO: Have a fast path that doesn't require
                        // recreating the bind group if only buffer contents
                        // change. For now, we just delete and recreate the bind
                        // group.
                        bind_group_allocator.free(*occupied_entry.get());
                        let new_binding =
                            bind_group_allocator.allocate_unprepared(unprepared, &material_layout);
                        *occupied_entry.get_mut() = new_binding;
                        new_binding
                    }
                    Entry::Vacant(vacant_entry) => *vacant_entry.insert(
                        bind_group_allocator.allocate_unprepared(unprepared, &material_layout),
                    ),
                }
            }
            Err(AsBindGroupError::RetryNextUpdate) => {
                return Err(PrepareAssetError::RetryNextUpdate(material))
            }
            Err(AsBindGroupError::CreateBindGroupDirectly) => {
                match material.as_bind_group(
                    &material_layout,
                    render_device,
                    pipeline_cache,
                    material_param,
                ) {
                    Ok(prepared_bind_group) => {
                        let bind_group_allocator =
                            bind_group_allocators.get_mut(&TypeId::of::<M>()).unwrap();
                        // Store the resulting bind group directly in the slot.
                        let material_binding_id =
                            bind_group_allocator.allocate_prepared(prepared_bind_group);
                        render_material_bindings.insert(material_id.into(), material_binding_id);
                        material_binding_id
                    }
                    Err(AsBindGroupError::RetryNextUpdate) => {
                        return Err(PrepareAssetError::RetryNextUpdate(material))
                    }
                    Err(other) => return Err(PrepareAssetError::AsBindGroupError(other)),
                }
            }
            Err(other) => return Err(PrepareAssetError::AsBindGroupError(other)),
        };

        let shadows_enabled = M::enable_shadows();
        let prepass_enabled = M::enable_prepass();

        let draw_opaque_pbr = opaque_draw_functions.read().id::<DrawPointCloudMaterial>();
        let draw_alpha_mask_pbr = alpha_mask_draw_functions
            .read()
            .id::<DrawPointCloudMaterial>();
        let draw_transmissive_pbr = transmissive_draw_functions
            .read()
            .id::<DrawPointCloudMaterial>();
        let draw_transparent_pbr = transparent_draw_functions
            .read()
            .id::<DrawPointCloudMaterial>();
        // let draw_opaque_multipass = multipass_draw_functions.read().id::<DrawMultipass>();
        let draw_opaque_prepass = opaque_prepass_draw_functions.read().id::<DrawPrepass>();
        let draw_alpha_mask_prepass = alpha_mask_prepass_draw_functions.read().id::<DrawPrepass>();
        let draw_opaque_prepass_depth_only = opaque_prepass_draw_functions
            .read()
            .id::<DrawDepthOnlyPrepass>();
        let draw_opaque_deferred = opaque_deferred_draw_functions.read().id::<DrawPrepass>();
        let draw_alpha_mask_deferred = alpha_mask_deferred_draw_functions
            .read()
            .id::<DrawPrepass>();
        let draw_shadows = shadow_draw_functions.read().id::<DrawPrepass>();
        let draw_shadows_depth_only = shadow_draw_functions.read().id::<DrawDepthOnlyPrepass>();

        let draw_functions = SmallVec::from_iter([
            (MainPassOpaqueDrawFunction.intern(), draw_opaque_pbr),
            (MainPassAlphaMaskDrawFunction.intern(), draw_alpha_mask_pbr),
            (
                MainPassTransmissiveDrawFunction.intern(),
                draw_transmissive_pbr,
            ),
            (
                MainPassTransparentDrawFunction.intern(),
                draw_transparent_pbr,
            ),
            // (MultiPassOpaqueDrawFunction.intern(), draw_opaque_multipass),
            (PrepassOpaqueDrawFunction.intern(), draw_opaque_prepass),
            (
                PrepassAlphaMaskDrawFunction.intern(),
                draw_alpha_mask_prepass,
            ),
            (
                PrepassOpaqueDepthOnlyDrawFunction.intern(),
                draw_opaque_prepass_depth_only,
            ),
            (DeferredOpaqueDrawFunction.intern(), draw_opaque_deferred),
            (
                DeferredAlphaMaskDrawFunction.intern(),
                draw_alpha_mask_deferred,
            ),
            (ShadowsDrawFunction.intern(), draw_shadows),
            (
                ShadowsDepthOnlyDrawFunction.intern(),
                draw_shadows_depth_only,
            ),
        ]);

        let render_method = match material.opaque_render_method() {
            OpaqueRendererMethod::Forward | OpaqueRendererMethod::Auto => {
                OpaqueRendererMethod::Forward
            }
            OpaqueRendererMethod::Deferred => OpaqueRendererMethod::Deferred,
            // OpaqueRendererMethod::Auto => default_opaque_render_method.0,
        };

        let mut mesh_pipeline_key_bits = MeshPipelineKey::empty();
        mesh_pipeline_key_bits.set(
            MeshPipelineKey::READS_VIEW_TRANSMISSION_TEXTURE,
            material.reads_view_transmission_texture(),
        );

        let reads_view_transmission_texture =
            mesh_pipeline_key_bits.contains(MeshPipelineKey::READS_VIEW_TRANSMISSION_TEXTURE);

        let mesh_pipeline_key_bits = ErasedMeshPipelineKey::new(mesh_pipeline_key_bits);

        let render_phase_type = match material.alpha_mode() {
            AlphaMode::Blend | AlphaMode::Premultiplied | AlphaMode::Add | AlphaMode::Multiply => {
                RenderPhaseType::Transparent
            }
            _ if reads_view_transmission_texture => RenderPhaseType::Transmissive,
            AlphaMode::Opaque | AlphaMode::AlphaToCoverage => RenderPhaseType::Opaque,
            AlphaMode::Mask(_) => RenderPhaseType::AlphaMask,
        };

        let mut shaders = SmallVec::new();
        let mut add_shader = |label: InternedShaderLabel, shader_ref: ShaderRef| {
            let mayber_shader = match shader_ref {
                ShaderRef::Default => None,
                ShaderRef::Handle(handle) => Some(handle),
                ShaderRef::Path(path) => Some(asset_server.load(path)),
            };
            if let Some(shader) = mayber_shader {
                shaders.push((label, shader));
            }
        };
        add_shader(MaterialVertexShader.intern(), M::vertex_shader());
        add_shader(MaterialFragmentShader.intern(), M::fragment_shader());
        add_shader(PrepassVertexShader.intern(), M::prepass_vertex_shader());
        add_shader(PrepassFragmentShader.intern(), M::prepass_fragment_shader());
        add_shader(DeferredVertexShader.intern(), M::deferred_vertex_shader());
        add_shader(
            DeferredFragmentShader.intern(),
            M::deferred_fragment_shader(),
        );

        let bindless = material_uses_bindless_resources::<M>(render_device);
        let bind_group_data = material.bind_group_data();
        let material_key = ErasedMaterialKey::new(bind_group_data);

        let view_settings_layout_entry =
            uniform_buffer::<<M::ViewSettings as ViewSettings>::Data>(false)
                .build(3, ShaderStages::VERTEX_FRAGMENT);

        Ok(PreparedPointCloudMaterial {
            binding,
            properties: Arc::new(PointCloudMaterialProperties {
                alpha_mode: material.alpha_mode(),
                depth_bias: material.depth_bias(),
                reads_view_transmission_texture,
                render_phase_type,
                render_method,
                mesh_pipeline_key_bits,
                material_layout: Some(material_layout),
                draw_functions,
                shaders,
                bindless,
                base_specialize: Some(base_specialize),
                prepass_specialize: Some(prepass_specialize),
                user_specialize: Some(user_specialize::<M>),
                material_key,
                shadows_enabled,
                prepass_enabled,
                view_settings_layout_entry: Some(view_settings_layout_entry),
            }),
        })
    }

    fn unload_asset(
        source_asset: AssetId<Self::SourceAsset>,
        (_, _, bind_group_allocators, render_material_bindings, ..): &mut SystemParamItem<
            Self::Param,
        >,
    ) {
        let Some(material_binding_id) = render_material_bindings.remove(&source_asset.untyped())
        else {
            return;
        };
        let bind_group_allactor = bind_group_allocators.get_mut(&TypeId::of::<M>()).unwrap();
        bind_group_allactor.free(material_binding_id);
    }
}

/// Stores the settings key computed for each material, used for specialization.
#[derive(Component, Default)]
pub struct ErasedMaterialViewSettingsKeys {
    pub view_settings_keys: HashMap<TypeId, ErasedViewSettingsKey, NoOpHash>,
}

#[derive(Component, Default, Deref, DerefMut)]
pub struct ErasedMaterialPreparedPasses(HashMap<TypeId, PreparedPasses, NoOpHash>);

#[derive(Default, Deref, DerefMut)]
pub struct PreparedPasses(HashMap<usize, Arc<PassProperties>, NoOpHash>);

/// Prepare erased view settings key and mark view as dirty if it changed.
/// TODO: do something for key changes on lights ?
pub fn prepare_material_view_settings_key_and_passes<M: PointCloudMaterial>(
    mut point_cloud_dirty_specializations: ResMut<PointCloudDirtySpecializations>,
    mut views: Query<
        (
            &M::ViewSettings,
            &mut ErasedMaterialViewSettingsKeys,
            Option<&mut ErasedMaterialPreparedPasses>,
            Option<&ExtractedView>,
            Option<&mut ViewMultipassTextures<M>>,
            &mut ViewPointCloudBindGroups,
        ),
        Changed<M::ViewSettings>,
    >,
    asset_server: Res<AssetServer>,
) {
    let type_id = TypeId::of::<M>();

    for (
        view_settings,
        mut erased_material_view_settings_keys,
        maybe_erased_material_prepared_passes,
        maybe_extracted_view,
        maybe_multipass_textures,
        mut view_point_cloud_bind_groups,
    ) in &mut views
    {
        let view_settings_key = view_settings.pipeline_key();
        let erased_view_settings_key = ErasedViewSettingsKey::new(view_settings_key.clone());

        let maybe_previous_key = erased_material_view_settings_keys
            .view_settings_keys
            .insert(type_id, erased_view_settings_key.clone());

        if maybe_previous_key.is_none() || !maybe_previous_key.eq(&Some(erased_view_settings_key)) {
            // if the key is on a view (camera), we have to (re-)prepare passes
            if let (Some(extracted_view), Some(mut material_prepared_passes)) =
                (maybe_extracted_view, maybe_erased_material_prepared_passes)
            {
                // clear all previous passes
                let mut prepared_passes =
                    HashMap::<usize, Arc<PassProperties>, NoOpHash>::default();

                // prepare the passes
                for pass in M::passes() {
                    match pass.kind {
                        PassKind::Geometry => {
                            if let Some(prepared_pass) =
                                M::prepare_geometry_pass(&pass.id, &view_settings_key)
                            {
                                let mut shaders = SmallVec::new();
                                let mut add_shader =
                                    |label: InternedShaderLabel, shader_ref: ShaderRef| {
                                        let mayber_shader = match shader_ref {
                                            ShaderRef::Default => None,
                                            ShaderRef::Handle(handle) => Some(handle),
                                            ShaderRef::Path(path) => Some(asset_server.load(path)),
                                        };
                                        if let Some(shader) = mayber_shader {
                                            shaders.push((label, shader));
                                        }
                                    };

                                add_shader(
                                    MaterialVertexShader.intern(),
                                    clone_shader_ref(&prepared_pass.vertex_shader),
                                );
                                add_shader(
                                    MaterialFragmentShader.intern(),
                                    clone_shader_ref(&prepared_pass.fragment_shader),
                                );

                                prepared_passes.insert(
                                    pass.id.into(),
                                    Arc::new(PassProperties {
                                        label: pass.label,
                                        kind: pass.kind,
                                        shaders,
                                        inputs: prepared_pass.inputs.clone(),
                                        output: prepared_pass.output.clone(),
                                        blend: prepared_pass.blend,
                                        depth_write_enabled: prepared_pass.depth_write_enabled,
                                    }),
                                );
                            };
                        }
                        PassKind::Fullscreen => {
                            if let Some(prepared_pass) =
                                M::prepare_fullscreen_pass(&pass.id, &view_settings_key)
                            {
                                let mut shaders = SmallVec::new();
                                let mut add_shader =
                                    |label: InternedShaderLabel, shader_ref: ShaderRef| {
                                        let mayber_shader = match shader_ref {
                                            ShaderRef::Default => None,
                                            ShaderRef::Handle(handle) => Some(handle),
                                            ShaderRef::Path(path) => Some(asset_server.load(path)),
                                        };
                                        if let Some(shader) = mayber_shader {
                                            shaders.push((label, shader));
                                        }
                                    };

                                add_shader(
                                    MaterialVertexShader.intern(),
                                    clone_shader_ref(&prepared_pass.vertex_shader),
                                );
                                add_shader(
                                    MaterialFragmentShader.intern(),
                                    clone_shader_ref(&prepared_pass.fragment_shader),
                                );

                                prepared_passes.insert(
                                    pass.id.into(),
                                    Arc::new(PassProperties {
                                        label: pass.label,
                                        kind: pass.kind,
                                        shaders,
                                        inputs: prepared_pass.inputs.clone(),
                                        output: prepared_pass.output.clone(),
                                        blend: prepared_pass.blend,
                                        depth_write_enabled: prepared_pass.depth_write_enabled,
                                    }),
                                );
                            };
                        }
                    }
                }

                if !prepared_passes.is_empty() {
                    material_prepared_passes.insert(type_id, PreparedPasses(prepared_passes));
                    // cleanup bind groups of no multipass mode
                    view_point_cloud_bind_groups.bind_groups.remove(&type_id);
                } else {
                    // cleanup passes
                    material_prepared_passes.remove(&type_id);

                    // clear not needed anymore multipass texture
                    if let Some(mut multipass_textures) = maybe_multipass_textures {
                        multipass_textures.textures.clear();
                        multipass_textures.has_changed = true;
                    }
                }

                // if the key has changed, mark the view as dirty to force respecialization
                point_cloud_dirty_specializations
                    .views
                    .insert(extracted_view.retained_view_entity);
            }
        }
    }
}

/// Prepare view settings uniform of each cameras having a view settings.
pub fn prepare_view_settings_uniforms<M: PointCloudMaterial>(
    mut commands: Commands,
    views: Query<
        (
            Entity,
            &M::ViewSettings,
            Option<&mut PreparedViewSettingsUniform<M>>,
        ),
        // TODO: keep this "Changed" filter ? Handle RemovedComponents ?
        (With<ExtractedView>, Changed<M::ViewSettings>),
    >,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    for (entity, view_settings, mut maybe_prepared_view_settings_uniform) in views {
        // Fetch or create the `PreparedViewSettingsUniform` component
        let mut prepared_view_settings_uniform = match maybe_prepared_view_settings_uniform {
            Some(ref mut prepared_view_settings_uniform) => {
                std::mem::take(&mut **prepared_view_settings_uniform)
            }
            None => PreparedViewSettingsUniform::default(),
        };

        prepared_view_settings_uniform
            .view_settings_uniform_buffer
            .set(view_settings.to_data());

        prepared_view_settings_uniform
            .view_settings_uniform_buffer
            .write_buffer(&render_device, &render_queue);

        let mut entity_commands = commands.entity(entity);
        entity_commands.insert(prepared_view_settings_uniform);
    }
}

/// For each lights, add [`PreparedViewSettingsUniform`] for each of its cascade.
/// This is needed because the extracted [`M::ViewSettings`] is on the extracted light, and not on
/// its cascades.
pub fn prepare_cascade_view_settings_uniforms<M: PointCloudMaterial>(
    mut commands: Commands,
    lights: Query<(Entity, &M::ViewSettings, &DirectionalLightViewEntities)>,
    mut views: Query<Option<&mut PreparedViewSettingsUniform<M>>, With<ExtractedView>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    for (_light_entity, view_settings, directional_light_view_entities) in lights {
        for (_view_entity, cascades) in directional_light_view_entities.iter() {
            for &cascade_entity in cascades {
                let Ok(mut maybe_prepared_view_settings_uniform) = views.get_mut(cascade_entity)
                else {
                    // this should never happen except if a cascade does'nt have the `ExtractedView`
                    // component... which should'nt happen.
                    continue;
                };

                // Fetch or create the `PreparedViewSettingsUniform` component
                let mut prepared_view_settings_uniform = match maybe_prepared_view_settings_uniform
                {
                    Some(ref mut prepared_view_settings_uniform) => {
                        std::mem::take(&mut **prepared_view_settings_uniform)
                    }
                    None => PreparedViewSettingsUniform::default(),
                };

                prepared_view_settings_uniform
                    .view_settings_uniform_buffer
                    .set(view_settings.to_data());

                prepared_view_settings_uniform
                    .view_settings_uniform_buffer
                    .write_buffer(&render_device, &render_queue);

                let mut entity_commands = commands.entity(cascade_entity);
                entity_commands.insert(prepared_view_settings_uniform);
            }
        }
    }
}

pub fn prepare_view_point_cloud_bind_groups<M: PointCloudMaterial>(
    pipeline_cache: Res<PipelineCache>,
    point_cloud_pipeline: Res<PointCloudPipeline>,
    render_device: Res<RenderDevice>,
    views: Query<(
        &ErasedMaterialPreparedPasses,
        &VisibleNodesTexture,
        &PreparedViewSettingsUniform<M>,
        &mut ViewPointCloudBindGroups,
    )>,
    prepared_point_cloud_uniforms: Res<PreparedPointCloudUniforms>,
    items: Query<&MainEntity, (With<PointCloud3d>, With<PointCloudMaterial3d<M>>)>,
) {
    let material_type_id = TypeId::of::<M>();

    // TODO: compute it once and store in a resource
    let mut layout = point_cloud_pipeline.point_cloud_layout.clone();
    layout.entries.push(
        uniform_buffer::<<M::ViewSettings as ViewSettings>::Data>(false)
            .build(3, ShaderStages::VERTEX_FRAGMENT),
    );

    let layout = &pipeline_cache.get_bind_group_layout(&layout);

    for (
        material_prepared_passes,
        visible_nodes_texture,
        prepared_view_settings_uniform,
        mut view_point_cloud_bind_groups,
    ) in views
    {
        // if there is prepared passes, we do not have to create the bind groups
        if material_prepared_passes.get(&material_type_id).is_some() {
            continue;
        };

        let bind_groups = view_point_cloud_bind_groups
            .bind_groups
            .entry(material_type_id)
            .or_default();

        for main_entity in items {
            let Some(prepared_uniform) = prepared_point_cloud_uniforms.get(main_entity) else {
                // warn!(
                //     "Missing prepared prepared_point_cloud_uniforms for entity {:?}",
                //     main_entity
                // );
                continue;
            };

            if visible_nodes_texture.has_changed {
                // first remove the previous bind group
                bind_groups.remove(main_entity);

                // then create a new if a texture is available
                if let Some(ref texture) = visible_nodes_texture.texture {
                    let bind_group = render_device.create_bind_group(
                        "view_point_cloud",
                        layout,
                        &BindGroupEntries::sequential((
                            prepared_uniform.mesh_uniform_buffer.binding().unwrap(),
                            &prepared_uniform.point_cloud_uniform_buffer,
                            &texture.default_view,
                            &prepared_view_settings_uniform.view_settings_uniform_buffer,
                        )),
                    );

                    bind_groups.insert(*main_entity, bind_group);
                }
            }
        }
    }
}
