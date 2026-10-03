mod components;
mod draw;
mod extract;
mod key;
mod pass;
mod pipeline;
mod prepare;
mod queue;
mod resources;
mod specialization;
mod specializer;

use bevy::{
    app::{App, First, Plugin, PostUpdate},
    asset::{Asset, AssetApp, AssetEventSystems},
    camera::Camera,
    core_pipeline::core_3d::{AlphaMask3d, Opaque3d, Transparent3d},
    ecs::prelude::*,
    light::DirectionalLight,
    material::{
        labels::{DrawFunctionLabel, ShaderLabel},
        AlphaMode, OpaqueRendererMethod,
    },
    mesh::{mark_3d_meshes_as_changed_if_their_assets_changed, MeshVertexBufferLayoutRef},
    pbr::{
        check_views_lights_need_specialization, collect_meshes_for_gpu_building, prepare_lights,
        set_mesh_motion_vector_flags, MaterialBindGroupAllocator, MaterialBindGroupAllocators,
        MaterialBindingId, Shadow, Transmissive3d,
    },
    render::{
        camera::{
            clear_dirty_wireframe_specializations, expire_wireframe_specializations_for_views,
            DirtySpecializationSystems, ExtractedCamera,
        },
        erased_render_asset::ErasedRenderAssetPlugin,
        extract_component::ExtractComponentPlugin,
        mesh::RenderMesh,
        prelude::*,
        render_asset::prepare_assets,
        render_phase::*,
        render_resource::*,
        renderer::RenderDevice,
        sync_world::RenderEntity,
        view::ExtractedView,
        GpuResourceAppExt, Render, RenderApp, RenderDebugFlags, RenderStartup, RenderSystems,
    },
    shader::ShaderRef,
};
use core::{any::TypeId, hash::Hash, marker::PhantomData};
use std::{
    fmt::Debug,
    sync::{Arc, OnceLock},
};

use crate::{
    render::{
        clear_dirty_specializations, expire_specializations_for_views,
        multipass::{
            MultipassMaterialPlugin, MultipassMaterialsPlugin, MultipassPipelinePlugin,
            ViewMultipassTextures,
        },
        prepass::{DrawDepthOnlyPrepass, DrawPrepass, PrepassPipelinePlugin, PrepassPlugin},
        queue_shadows, specialize_shadows, PendingShadowQueues, PointCloudDirtySpecializations,
        PointCloudPipelineSystems, SpecializedPointCloudPipelines,
        SpecializedShadowMaterialPipelineCache,
    },
    PointCloudChunk3d, PointCloudMaterial3d, ViewSettings,
};

pub use components::*;
pub use draw::*;
pub use extract::*;
pub use key::*;
pub use pass::*;
pub use pipeline::*;
pub use prepare::*;
pub use queue::*;
pub use resources::*;
pub use specialization::*;
pub use specializer::*;

/// Materials are used alongside [`PointCloudMaterialPlugin`], [`PointCloud3d`], and
/// [`PointCloudMaterial3d`] to spawn entities that are rendered with a specific [`Material`] type.
/// They serve as an easy to use high level way to render [`PointCloud3d`] entities with custom
/// shader logic.
///
/// Materials must implement [`AsBindGroup`] to define how data will be transferred to the GPU and
/// bound in shaders. [`AsBindGroup`] can be derived, which makes generating bindings
/// straightforward. See the [`AsBindGroup`] docs for details.
///
/// Materials must also define a [`PointCloudMaterial::ViewSettings`] type for view based settings
/// (eg: clipping planes, multipass materials, ...).
pub trait PointCloudMaterial: Asset + AsBindGroup + Clone + Sized {
    type ViewSettings: ViewSettings;
    // type ViewSettings: Component
    //     + ExtractComponent
    //     + Clone
    //     + Copy
    //     + ShaderType
    //     + WriteInto
    //     + Default
    //     + ViewSettingsPipelineKey;

    /// Number of passes for multipass materials.
    /// If equal to 0, means that this is not a multipass material.
    const PASS_COUNT: usize = 0;

    /// Returns this material's vertex shader. If [`ShaderRef::Default`] is returned, the default
    /// mesh vertex shader will be used.
    fn vertex_shader() -> ShaderRef {
        ShaderRef::Default
    }

    /// Returns this material's fragment shader. If [`ShaderRef::Default`] is returned, the default
    /// mesh fragment shader will be used.
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Default
    }

    /// The passes in case this is a multipass material
    fn passes() -> &'static [PassDescriptor] {
        static PASSES: OnceLock<Vec<PassDescriptor>> = OnceLock::new();

        PASSES.get_or_init(Vec::new)
    }

    /// Returns this material's [`AlphaMode`]. Defaults to [`AlphaMode::Opaque`].
    #[inline]
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }

    /// Returns if this material should be rendered by the deferred or forward renderer.
    /// for `AlphaMode::Opaque` or `AlphaMode::Mask` materials.
    /// If `OpaqueRendererMethod::Auto`, it will default to what is selected in the
    /// `DefaultOpaqueRendererMethod` resource.
    #[inline]
    fn opaque_render_method(&self) -> OpaqueRendererMethod {
        OpaqueRendererMethod::Forward
    }

    #[inline]
    /// Add a bias to the view depth of the mesh which can be used to force a specific render order.
    /// for meshes with similar depth, to avoid z-fighting.
    /// The bias is in depth-texture units so large values may be needed to overcome small depth
    /// differences.
    fn depth_bias(&self) -> f32 {
        0.0
    }

    #[inline]
    /// Returns whether the material would like to read from [`ViewTransmissionTexture`].
    ///
    /// This allows taking color output from the [`Opaque3d`] pass as an input, (for screen-space
    /// transmission) but requires rendering to take place in a separate [`Transmissive3d`]
    /// pass.
    fn reads_view_transmission_texture(&self) -> bool {
        false
    }

    /// Controls if the prepass is enabled for the Material.
    /// For more information about what a prepass is, see the [`bevy_core_pipeline::prepass`] docs.
    #[inline]
    fn enable_prepass() -> bool {
        true
    }

    /// Controls if shadows are enabled for the Material.
    #[inline]
    fn enable_shadows() -> bool {
        true
    }

    /// Returns this material's prepass vertex shader. If [`ShaderRef::Default`] is returned, the
    /// default prepass vertex shader will be used.
    ///
    /// This is used for the various [prepasses](bevy_core_pipeline::prepass) as well as for
    /// generating the depth maps required for shadow mapping.
    fn prepass_vertex_shader() -> ShaderRef {
        ShaderRef::Default
    }

    /// Returns this material's prepass fragment shader. If [`ShaderRef::Default`] is returned, the
    /// default prepass fragment shader will be used.
    ///
    /// This is used for the various [prepasses](bevy_core_pipeline::prepass) as well as for
    /// generating the depth maps required for shadow mapping.
    fn prepass_fragment_shader() -> ShaderRef {
        ShaderRef::Default
    }

    /// Returns this material's deferred vertex shader. If [`ShaderRef::Default`] is returned, the
    /// default deferred vertex shader will be used.
    fn deferred_vertex_shader() -> ShaderRef {
        ShaderRef::Default
    }

    /// Returns this material's deferred fragment shader. If [`ShaderRef::Default`] is returned, the
    /// default deferred fragment shader will be used.
    fn deferred_fragment_shader() -> ShaderRef {
        ShaderRef::Default
    }

    /// Customizes the default [`RenderPipelineDescriptor`] for a specific entity using the entity's
    /// [`MaterialPipelineKey`] and [`MeshVertexBufferLayoutRef`] as input.
    #[expect(
        unused_variables,
        reason = "The parameters here are intentionally unused by the default implementation; however, putting underscores here will result in the underscores being copied by rust-analyzer's tab completion."
    )]
    #[inline]
    fn specialize(
        pipeline: &PointCloudMaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        splat_layout: &MeshVertexBufferLayoutRef,
        instance_layout: &MeshVertexBufferLayoutRef,
        key: PointCloudMaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        Ok(())
    }
}

#[derive(Default)]
pub struct MaterialsPlugin {
    /// Debugging flags that can optionally be set when constructing the renderer.
    pub debug_flags: RenderDebugFlags,
}

impl Plugin for MaterialsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            PrepassPipelinePlugin,
            PrepassPlugin::new(self.debug_flags),
            MultipassPipelinePlugin,
            MultipassMaterialsPlugin::new(self.debug_flags),
        ));

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                // From camera
                .init_resource::<PointCloudDirtySpecializations>()
                .configure_sets(
                    ExtractSchedule,
                    (
                        DirtySpecializationSystems::Clear
                            .before(DirtySpecializationSystems::CheckForChanges),
                        DirtySpecializationSystems::CheckForChanges
                            .before(DirtySpecializationSystems::CheckForRemovals),
                    ),
                )
                .add_systems(
                    ExtractSchedule,
                    (
                        clear_dirty_specializations.in_set(DirtySpecializationSystems::Clear),
                        clear_dirty_wireframe_specializations
                            .in_set(DirtySpecializationSystems::Clear),
                        expire_specializations_for_views.in_set(RenderSystems::Cleanup),
                        expire_wireframe_specializations_for_views.in_set(RenderSystems::Cleanup),
                    ),
                )
                // End camera
                .init_gpu_resource::<SpecializedPointCloudMaterialPipelineCache>()
                .init_gpu_resource::<SpecializedPointCloudPipelines<PointCloudMaterialPipelineSpecializer>>()
                // .init_gpu_resource::<LightKeyCache>()
                .init_gpu_resource::<SpecializedShadowMaterialPipelineCache>()
                // .init_resource::<DrawFunctions<Shadow>>()
                .init_resource::<RenderPointCloudMaterialInstances>()
                .allow_ambiguous_resource::<RenderPointCloudMaterialInstances>()
                .init_resource::<MaterialBindGroupAllocators>()
                .allow_ambiguous_resource::<MaterialBindGroupAllocators>()
                .init_gpu_resource::<PendingPointCloudMaterialQueues>()
                .allow_ambiguous_resource::<PendingPointCloudMaterialQueues>()
                .init_gpu_resource::<PendingShadowQueues>()
                .allow_ambiguous_resource::<PendingShadowQueues>()
                .add_render_command::<Shadow, DrawPrepass>()
                .add_render_command::<Shadow, DrawDepthOnlyPrepass>()
                .add_render_command::<Transparent3d, DrawPointCloudMaterial>()
                .add_render_command::<Opaque3d, DrawPointCloudMaterial>()
                .add_render_command::<AlphaMask3d, DrawPointCloudMaterial>()
                .add_render_command::<Transmissive3d, DrawPointCloudMaterial>()
                .add_systems(
                    RenderStartup,
                    init_point_cloud_material_pipeline.after(PointCloudPipelineSystems),
                )
                .add_systems(
                    Render,
                    (
                        specialize_point_cloud_materials
                            .in_set(RenderSystems::Specialize)
                            .after(prepare_assets::<RenderMesh>)
                            .after(collect_meshes_for_gpu_building)
                            .after(set_mesh_motion_vector_flags),
                        queue_material_meshes.in_set(RenderSystems::QueueMeshes),
                    ),
                )
                .add_systems(
                    Render,
                    (
                        check_views_lights_need_specialization
                            .in_set(RenderSystems::Specialize)
                            .before(specialize_shadows),
                        // specialize_shadows also needs to run after
                        // prepare_assets::<PreparedMaterial>,
                        // which is fine since Specialize is after PrepareAssets
                        specialize_shadows
                            .in_set(RenderSystems::Specialize)
                            .after(prepare_lights),
                        queue_shadows.in_set(RenderSystems::QueueMeshes),
                    ),
                );

            render_app
                .world_mut()
                .register_required_components::<ExtractedView, ErasedMaterialViewSettingsKeys>();
        }
    }
}

/// Adds the necessary ECS resources and render logic to enable rendering entities using the given
/// [`Material`] asset type.
pub struct PointCloudMaterialPlugin<M: PointCloudMaterial> {
    /// Debugging flags that can optionally be set when constructing the renderer.
    pub debug_flags: RenderDebugFlags,
    pub _marker: PhantomData<M>,
}

impl<M: PointCloudMaterial> Default for PointCloudMaterialPlugin<M> {
    fn default() -> Self {
        Self {
            debug_flags: RenderDebugFlags::default(),
            _marker: Default::default(),
        }
    }
}

impl<M: PointCloudMaterial> Plugin for PointCloudMaterialPlugin<M>
where
    M::Data: PartialEq + Eq + Hash + Clone,
{
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<PointCloudMaterial3d<M>>::default());

        if !app.is_plugin_added::<ExtractComponentPlugin<M::ViewSettings>>() {
            app.add_plugins(ExtractComponentPlugin::<M::ViewSettings>::default());
        }

        app.init_asset::<M>()
            .register_type::<PointCloudMaterial3d<M>>()
            .init_resource::<EntitiesNeedingSpecialization<M>>()
            .add_plugins(ErasedRenderAssetPlugin::<PointCloudMaterial3d<M>>::default())
            .add_systems(First, clear_entities_needing_specialization_removed::<M>)
            .add_systems(
                PostUpdate,
                check_entities_needing_specialization::<M>
                    .after(AssetEventSystems)
                    .after(mark_3d_meshes_as_changed_if_their_assets_changed),
            )
            .add_observer(on_remove_point_cloud_chunk_3d::<M>)
            .add_observer(on_remove_point_cloud_material_3d::<M>);

        app.world_mut()
            .register_required_components::<Camera, M::ViewSettings>();

        app.world_mut()
            .register_required_components::<DirectionalLight, M::ViewSettings>();

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<PointCloudMaterialTargets<M>>()
                .add_systems(RenderStartup, add_material_bind_group_allocator::<M>)
                .add_systems(
                    ExtractSchedule,
                    (
                        extract_point_cloud_materials::<M>.in_set(MaterialExtractionSystems),
                        early_sweep_material_instances::<M>
                            .after(MaterialExtractionSystems)
                            .before(late_sweep_material_instances),
                        extract_entities_needs_specialization::<M>
                            .in_set(DirtySpecializationSystems::CheckForChanges),
                        extract_entities_that_need_specializations_removed::<M>
                            .in_set(DirtySpecializationSystems::CheckForRemovals),
                        free_removed_point_cloud_materials::<M>,
                    ),
                )
                .add_systems(
                    Render,
                    (
                        // must be run before specialize
                        prepare_material_view_settings_key::<M>
                            .in_set(RenderSystems::PrepareAssets),
                        (
                            prepare_view_settings_uniforms::<M>,
                            prepare_cascade_view_settings_uniforms::<M>,
                        )
                            .in_set(RenderSystems::PrepareResources),
                        prepare_view_point_cloud_bind_groups::<M>
                            .in_set(RenderSystems::PrepareBindGroups),
                    ),
                );

            render_app
                .world_mut()
                .register_required_components::<ExtractedCamera, ViewMultipassTextures<M>>();

            // TODO: conditionnaly add if the material is multipass
            if M::PASS_COUNT > 0 {
                app.add_plugins(MultipassMaterialPlugin::<M>::new(self.debug_flags));
            }
        }
    }
}

/// Returns true if the material will *actually* use bindless resources or false
/// if it won't.
///
/// This takes the platform support (or lack thereof) for bindless resources
/// into account.
pub fn material_uses_bindless_resources<M>(render_device: &RenderDevice) -> bool
where
    M: PointCloudMaterial,
{
    M::bindless_slot_count().is_some_and(|bindless_slot_count| {
        M::bindless_supported(render_device) && bindless_slot_count.resolve() > 1
    })
}

fn add_material_bind_group_allocator<M: PointCloudMaterial>(
    render_device: Res<RenderDevice>,
    mut bind_group_allocators: ResMut<MaterialBindGroupAllocators>,
) {
    // do not erase an existing bind group allocator
    if !bind_group_allocators.contains_key(&TypeId::of::<M>()) {
        bind_group_allocators.insert(
            TypeId::of::<M>(),
            MaterialBindGroupAllocator::new(
                &render_device,
                M::label(),
                material_uses_bindless_resources::<M>(&render_device)
                    .then(|| M::bindless_descriptor())
                    .flatten(),
                M::bind_group_layout_descriptor(&render_device),
                M::bindless_slot_count(),
            ),
        );
    }
}

// /// A dummy [`AssetId`] that we use as a placeholder whenever a mesh doesn't
// /// have a material.
// ///
// /// See the comments in [`RenderMaterialInstances::mesh_material`] for more
// /// information.
// pub(crate) static DUMMY_MESH_MATERIAL: AssetId<StandardMaterial> =
//     AssetId::<StandardMaterial>::invalid();

/// A [`SystemSet`] that contains all `extract_mesh_materials` systems.
#[derive(SystemSet, Clone, PartialEq, Eq, Debug, Hash)]
pub struct MaterialExtractionSystems;

pub(crate) fn on_remove_point_cloud_chunk_3d<M: PointCloudMaterial>(
    event: On<Remove, PointCloudChunk3d>,
    mapper: Query<&RenderEntity>,
    mut entities_need_specialization: ResMut<EntitiesNeedingSpecialization<M>>,
) {
    if let Ok(render_entity) = mapper.get(event.entity) {
        entities_need_specialization
            .removed
            .insert(event.entity, *render_entity);
    }
}

pub fn on_remove_point_cloud_material_3d<M: PointCloudMaterial>(
    event: On<Remove, PointCloudMaterial3d<M>>,
    mapper: Query<&RenderEntity>,
    mut entities_need_specialization: ResMut<EntitiesNeedingSpecialization<M>>,
) {
    if let Ok(render_entity) = mapper.get(event.entity) {
        entities_need_specialization
            .removed
            .insert(event.entity, *render_entity);
    }
}

#[derive(ShaderLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct MaterialVertexShader;

#[derive(ShaderLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct MaterialFragmentShader;

#[derive(ShaderLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct PrepassVertexShader;

#[derive(ShaderLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct PrepassFragmentShader;

#[derive(ShaderLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct DeferredVertexShader;

#[derive(ShaderLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct DeferredFragmentShader;

#[derive(ShaderLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct MeshletFragmentShader;

#[derive(ShaderLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct MeshletPrepassFragmentShader;

#[derive(ShaderLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct MeshletDeferredFragmentShader;

#[derive(DrawFunctionLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct MainPassOpaqueDrawFunction;
#[derive(DrawFunctionLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct MainPassAlphaMaskDrawFunction;
#[derive(DrawFunctionLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct MainPassTransmissiveDrawFunction;
#[derive(DrawFunctionLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct MainPassTransparentDrawFunction;

#[derive(DrawFunctionLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct MultiPassOpaqueDrawFunction;

#[derive(DrawFunctionLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct PrepassOpaqueDrawFunction;
#[derive(DrawFunctionLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct PrepassAlphaMaskDrawFunction;
#[derive(DrawFunctionLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct PrepassOpaqueDepthOnlyDrawFunction;

#[derive(DrawFunctionLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct DeferredOpaqueDrawFunction;
#[derive(DrawFunctionLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct DeferredAlphaMaskDrawFunction;

#[derive(DrawFunctionLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct ShadowsDrawFunction;
#[derive(DrawFunctionLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct ShadowsDepthOnlyDrawFunction;

/// Data prepared for a [`Material`] instance.
pub struct PreparedPointCloudMaterial {
    pub binding: MaterialBindingId,
    pub properties: Arc<PointCloudMaterialProperties>,
}

fn clone_shader_ref(vertex_shader: &ShaderRef) -> ShaderRef {
    match vertex_shader {
        ShaderRef::Default => ShaderRef::Default,
        ShaderRef::Handle(handle) => ShaderRef::Handle(handle.clone()),
        ShaderRef::Path(asset_path) => ShaderRef::Path(asset_path.clone()),
    }
}
