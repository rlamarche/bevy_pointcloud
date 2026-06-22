mod resources;
mod simple;

use std::{any::TypeId, hash::Hash, marker::PhantomData, sync::Arc};

use bevy_app::prelude::*;
use bevy_asset::{Asset, AssetApp, AssetId, AssetServer, Handle};
use bevy_camera::visibility::ViewVisibility;
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::{
    component::Component,
    prelude::*,
    reflect::ReflectComponent,
    system::{
        lifetimeless::{SRes, SResMut},
        Res, ResMut, SystemParamItem,
    },
};
use bevy_mesh::MeshVertexBufferLayoutRef;
use bevy_pbr::{
    ErasedMaterialKey, ErasedMaterialPipelineKey, MaterialBindGroupAllocator,
    MaterialBindGroupAllocators, MaterialBindingId, MaterialPipeline, RenderMaterialBindings,
};
use bevy_platform::collections::hash_map::Entry;
use bevy_reflect::{std_traits::ReflectDefault, Reflect};
use bevy_render::{
    erased_render_asset::{ErasedRenderAsset, ErasedRenderAssetPlugin, PrepareAssetError},
    extract_component::ExtractComponentPlugin,
    render_resource::{
        AsBindGroup, AsBindGroupError, BindGroupLayoutDescriptor, PipelineCache,
        RenderPipelineDescriptor, SpecializedMeshPipelineError,
    },
    renderer::RenderDevice,
    sync_world::MainEntity,
    Extract, ExtractSchedule, RenderApp, RenderDebugFlags, RenderStartup,
};
use bevy_shader::{Shader, ShaderDefVal, ShaderRef};
use derive_more::derive::From;
pub use resources::*;
pub use simple::*;

use crate::{
    point::{GpuPoint, Point},
    point_cloud::PointCloud3d,
    render::POINTCLOUD_SHADER_HANDLE,
};

pub enum RenderPass {
    Depth,
    Attribute,
}

pub trait PointCloudMaterial: Asset + AsBindGroup + Clone + Sized {
    fn vertex_shader(_: RenderPass) -> ShaderRef {
        ShaderRef::Default
    }

    fn fragment_shader(_: RenderPass) -> ShaderRef {
        ShaderRef::Default
    }

    fn normalize_shader() -> ShaderRef {
        ShaderRef::Default
    }

    fn shader_defs(_: RenderPass) -> Vec<ShaderDefVal> {
        Vec::new()
    }
}

#[derive(Component, Clone, Debug, Deref, DerefMut, Reflect, From)]
#[reflect(Component, Default, Clone, PartialEq)]
pub struct PointCloudMaterial3d<M: PointCloudMaterial>(pub Handle<M>);

impl<M: PointCloudMaterial> Default for PointCloudMaterial3d<M> {
    fn default() -> Self {
        Self(Handle::default())
    }
}

impl<M: PointCloudMaterial> PartialEq for PointCloudMaterial3d<M> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<M: PointCloudMaterial> Eq for PointCloudMaterial3d<M> {}

impl<M: PointCloudMaterial> From<PointCloudMaterial3d<M>> for AssetId<M> {
    fn from(point_cloud_material_3d: PointCloudMaterial3d<M>) -> Self {
        point_cloud_material_3d.id()
    }
}

impl<M: PointCloudMaterial> From<&PointCloudMaterial3d<M>> for AssetId<M> {
    fn from(point_cloud_material_3d: &PointCloudMaterial3d<M>) -> Self {
        point_cloud_material_3d.id()
    }
}

#[derive(Default)]
pub struct PointCloudMaterialsPlugin {
    /// Debugging flags that can optionally be set when constructing the renderer.
    pub debug_flags: RenderDebugFlags,
}

impl Plugin for PointCloudMaterialsPlugin {
    fn build(&self, app: &mut App) {
        // app.add_plugins((PrepassPipelinePlugin, PrepassPlugin::new(self.debug_flags)));
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                // .init_resource::<EntitySpecializationTicks>()
                // .init_resource::<SpecializedMaterialPipelineCache>()
                // .init_resource::<SpecializedMeshPipelines<MaterialPipelineSpecializer>>()
                // .init_resource::<LightKeyCache>()
                // .init_resource::<LightSpecializationTicks>()
                // .init_resource::<SpecializedShadowMaterialPipelineCache>()
                // .init_resource::<DrawFunctions<Shadow>>()
                .init_resource::<RenderPointCloudMaterialInstances>()
                // .init_resource::<MaterialBindGroupAllocators>()
                // .add_render_command::<Shadow, DrawPrepass>()
                // .add_render_command::<Transmissive3d, DrawMaterial>()
                // .add_render_command::<Transparent3d, DrawMaterial>()
                // .add_render_command::<Opaque3d, DrawMaterial>()
                // .add_render_command::<AlphaMask3d, DrawMaterial>()
                // .add_systems(RenderStartup, init_material_pipeline)
                // .add_systems(
                //     Render,
                //     (
                //         specialize_material_meshes
                //             .in_set(RenderSystems::PrepareMeshes)
                //             .after(prepare_assets::<RenderMesh>)
                //             .after(collect_meshes_for_gpu_building)
                //             .after(set_mesh_motion_vector_flags),
                //         queue_material_meshes.in_set(RenderSystems::QueueMeshes),
                //     ),
                // )
                // .add_systems(
                //     Render,
                //     (
                //         prepare_material_bind_groups,
                //         write_material_bind_group_buffers,
                //     )
                //         .chain()
                //         .in_set(RenderSystems::PrepareBindGroups),
                // )
                // .add_systems(
                //     Render,
                //     (
                //         check_views_lights_need_specialization.in_set(RenderSystems::PrepareAssets),
                //         // specialize_shadows also needs to run after prepare_assets::<PreparedMaterial>,
                //         // which is fine since ManageViews is after PrepareAssets
                //         specialize_shadows
                //             .in_set(RenderSystems::ManageViews)
                //             .after(prepare_lights),
                //         queue_shadows.in_set(RenderSystems::QueueMeshes),
                //     ),
                // )
            ;
        }
    }
}

#[allow(clippy::type_complexity)]
pub struct PointCloudMaterialPlugin<T: Point, U: GpuPoint, M: PointCloudMaterial>(
    PhantomData<fn() -> (T, U, M)>,
)
where
    for<'a> &'a T: Into<U>;

impl<T: Point, U: GpuPoint, M: PointCloudMaterial> Default for PointCloudMaterialPlugin<T, U, M>
where
    for<'a> &'a T: Into<U>,
{
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: Point, U: GpuPoint, M: PointCloudMaterial> Plugin for PointCloudMaterialPlugin<T, U, M>
where
    M::Data: PartialEq + Eq + Hash + Clone,
    for<'a> &'a T: Into<U>,
{
    fn build(&self, app: &mut App) {
        app.init_asset::<M>().add_plugins((
            ExtractComponentPlugin::<PointCloudMaterial3d<M>>::default(),
            ErasedRenderAssetPlugin::<PointCloudMaterial3d<M>>::default(),
        ));

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_systems(RenderStartup, add_material_bind_group_allocator::<M>)
                .add_systems(
                    ExtractSchedule,
                    (
                        extract_point_cloud_materials::<M>
                            .in_set(PointCloudMaterialExtractionSystems),
                        early_sweep_point_cloud_material_instances::<M>
                            .after(PointCloudMaterialExtractionSystems)
                            .before(late_sweep_point_cloud_material_instances::<T>),
                    ),
                );
        }
    }
}

/// A [`SystemSet`] that contains all `extract_mesh_materials` systems.
#[derive(SystemSet, Clone, PartialEq, Eq, Debug, Hash)]
pub struct PointCloudMaterialExtractionSystems;

fn add_material_bind_group_allocator<M: PointCloudMaterial>(
    render_device: Res<RenderDevice>,
    mut bind_group_allocators: ResMut<MaterialBindGroupAllocators>,
) {
    bind_group_allocators.insert(
        TypeId::of::<M>(),
        MaterialBindGroupAllocator::new(
            &render_device,
            M::label(),
            point_cloud_material_uses_bindless_resources::<M>(&render_device)
                .then(|| M::bindless_descriptor())
                .flatten(),
            M::bind_group_layout_descriptor(&render_device),
            M::bindless_slot_count(),
        ),
    );
}

/// Returns true if the material will *actually* use bindless resources or false
/// if it won't.
///
/// This takes the platform support (or lack thereof) for bindless resources
/// into account.
fn point_cloud_material_uses_bindless_resources<M>(render_device: &RenderDevice) -> bool
where
    M: PointCloudMaterial,
{
    M::bindless_slot_count().is_some_and(|bindless_slot_count| {
        M::bindless_supported(render_device) && bindless_slot_count.resolve() > 1
    })
}

/// Fills the [`RenderPointCloudMaterialInstances`] resources from the point clouds in the
/// scene.
#[allow(clippy::type_complexity)]
fn extract_point_cloud_materials<M: PointCloudMaterial>(
    mut material_instances: ResMut<RenderPointCloudMaterialInstances>,
    changed_meshes_query: Extract<
        Query<
            (Entity, &ViewVisibility, &PointCloudMaterial3d<M>),
            Or<(Changed<ViewVisibility>, Changed<PointCloudMaterial3d<M>>)>,
        >,
    >,
) {
    let last_change_tick = material_instances.current_change_tick;

    for (entity, view_visibility, material) in &changed_meshes_query {
        if view_visibility.get() {
            material_instances.instances.insert(
                entity.into(),
                RenderPointCloudMaterialInstance {
                    asset_id: material.id().untyped(),
                    last_change_tick,
                },
            );
        } else {
            material_instances
                .instances
                .remove(&MainEntity::from(entity));
        }
    }
}

/// Removes point cloud materials from [`RenderPointCloudMaterialInstances`] when their
/// [`PointCloudMaterial3d`] components are removed.
///
/// This is tricky because we have to deal with the case in which a material of
/// type A was removed and replaced with a material of type B in the same frame
/// (which is actually somewhat common of an operation). In this case, even
/// though an entry will be present in `RemovedComponents<PointCloudMaterial3d<M>>`,
/// we must not remove the entry in `RenderPointCloudMaterialInstances` which corresponds
/// to material B. To handle this case, we use change ticks to avoid removing
/// the entry if it was updated this frame.
///
/// This is the first of two sweep phases. Because this phase runs once per
/// material type, we need a second phase in order to guarantee that we only
/// bump [`RenderPointCloudMaterialInstances::current_change_tick`] once.
fn early_sweep_point_cloud_material_instances<M: PointCloudMaterial>(
    mut material_instances: ResMut<RenderPointCloudMaterialInstances>,
    mut removed_materials_query: Extract<RemovedComponents<PointCloudMaterial3d<M>>>,
) {
    let last_change_tick = material_instances.current_change_tick;

    for entity in removed_materials_query.read() {
        if let Entry::Occupied(occupied_entry) = material_instances.instances.entry(entity.into()) {
            // Only sweep the entry if it wasn't updated this frame.
            if occupied_entry.get().last_change_tick != last_change_tick {
                occupied_entry.remove();
            }
        }
    }
}

/// Removes point cloud materials from [`RenderPointCloudMaterialInstances`] when their
/// [`ViewVisibility`] components are removed.
///
/// This runs after all invocations of `early_sweep_point_cloud_material_instances` and is
/// responsible for bumping [`RenderPointCloudMaterialInstances::current_change_tick`] in
/// preparation for a new frame.
pub fn late_sweep_point_cloud_material_instances<T: Point>(
    mut material_instances: ResMut<RenderPointCloudMaterialInstances>,
    mut removed_meshes_query: Extract<RemovedComponents<PointCloud3d<T>>>,
) {
    let last_change_tick = material_instances.current_change_tick;

    for entity in removed_meshes_query.read() {
        if let Entry::Occupied(occupied_entry) = material_instances.instances.entry(entity.into()) {
            // Only sweep the entry if it wasn't updated this frame. It's
            // possible that a `ViewVisibility` component was removed and
            // re-added in the same frame.
            if occupied_entry.get().last_change_tick != last_change_tick {
                occupied_entry.remove();
            }
        }
    }

    material_instances
        .current_change_tick
        .set(last_change_tick.get() + 1);
}

/// Common [`Material`] properties, calculated for a specific material instance.
#[derive(Default)]
#[allow(clippy::type_complexity)]
pub struct PointCloudMaterialProperties {
    // pub render_phase_type: RenderPhaseType,
    pub material_layout: Option<BindGroupLayoutDescriptor>,
    /// Backing array is a size of 4 because the `StandardMaterial` needs 4 draw functions by default
    // pub draw_functions: SmallVec<[(InternedDrawFunctionLabel, DrawFunctionId); 4]>,
    /// Backing array is a size of 3 because the `StandardMaterial` has 3 custom shaders (`frag`, `prepass_frag`, `deferred_frag`) which is the
    /// most common use case
    // pub shaders: SmallVec<[(InternedShaderLabel, Handle<Shader>); 3]>,
    pub depth_pass_vertex_shader_handle: Handle<Shader>,
    pub depth_pass_fragment_shader_handle: Handle<Shader>,
    pub depth_shader_defs: Vec<ShaderDefVal>,
    pub attribute_pass_vertex_shader_handle: Handle<Shader>,
    pub attribute_pass_fragment_shader_handle: Handle<Shader>,
    pub attribute_shader_defs: Vec<ShaderDefVal>,
    pub normalize_shader_handle: Handle<Shader>,
    /// Whether this material *actually* uses bindless resources, taking the
    /// platform support (or lack thereof) of bindless resources into account.
    pub bindless: bool,
    pub specialize: Option<
        fn(
            &MaterialPipeline,
            &mut RenderPipelineDescriptor,
            &MeshVertexBufferLayoutRef,
            ErasedMaterialPipelineKey,
        ) -> Result<(), SpecializedMeshPipelineError>,
    >,
    /// The key for this material, typically a bitfield of flags that are used to modify
    /// the pipeline descriptor used for this material.
    pub material_key: ErasedMaterialKey,
}

impl PointCloudMaterialProperties {
    // pub fn get_shader(&self, label: impl ShaderLabel) -> Option<Handle<Shader>> {
    //     self.shaders
    //         .iter()
    //         .find(|(inner_label, _)| inner_label == &label.intern())
    //         .map(|(_, shader)| shader)
    //         .cloned()
    // }

    // pub fn add_shader(&mut self, label: impl ShaderLabel, shader: Handle<Shader>) {
    //     self.shaders.push((label.intern(), shader));
    // }

    // pub fn get_draw_function(&self, label: impl DrawFunctionLabel) -> Option<DrawFunctionId> {
    //     self.draw_functions
    //         .iter()
    //         .find(|(inner_label, _)| inner_label == &label.intern())
    //         .map(|(_, shader)| shader)
    //         .cloned()
    // }

    // pub fn add_draw_function(
    //     &mut self,
    //     label: impl DrawFunctionLabel,
    //     draw_function: DrawFunctionId,
    // ) {
    //     self.draw_functions.push((label.intern(), draw_function));
    // }
}

/// Data prepared for a [`Material`] instance.
pub struct PreparedPointCloudMaterial {
    pub binding: MaterialBindingId,
    pub properties: Arc<PointCloudMaterialProperties>,
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
        SResMut<MaterialBindGroupAllocators>,
        SResMut<RenderMaterialBindings>,
        SRes<AssetServer>,
        M::Param,
    );

    fn prepare_asset(
        material: Self::SourceAsset,
        material_id: AssetId<Self::SourceAsset>,
        (
            render_device,
            pipeline_cache,
            bind_group_allocators,
            render_material_bindings,
            asset_server,
            material_param,
        ): &mut SystemParamItem<Self::Param>,
    ) -> Result<
        Self::ErasedAsset,
        bevy_render::erased_render_asset::PrepareAssetError<Self::SourceAsset>,
    > {
        // let shadows_enabled = M::enable_shadows();
        // let prepass_enabled = M::enable_prepass();

        // let draw_opaque_pbr = opaque_draw_functions.read().id::<DrawMaterial>();
        // let draw_alpha_mask_pbr = alpha_mask_draw_functions.read().id::<DrawMaterial>();
        // let draw_transmissive_pbr = transmissive_draw_functions.read().id::<DrawMaterial>();
        // let draw_transparent_pbr = transparent_draw_functions.read().id::<DrawMaterial>();
        // let draw_opaque_prepass = opaque_prepass_draw_functions.read().id::<DrawPrepass>();
        // let draw_alpha_mask_prepass = alpha_mask_prepass_draw_functions.read().id::<DrawPrepass>();
        // let draw_opaque_deferred = opaque_deferred_draw_functions.read().id::<DrawPrepass>();
        // let draw_alpha_mask_deferred = alpha_mask_deferred_draw_functions
        //     .read()
        //     .id::<DrawPrepass>();
        // let draw_shadows = shadow_draw_functions.read().id::<DrawPrepass>();

        // let draw_functions = SmallVec::from_iter([
        //     (MainPassOpaqueDrawFunction.intern(), draw_opaque_pbr),
        //     (MainPassAlphaMaskDrawFunction.intern(), draw_alpha_mask_pbr),
        //     (
        //         MainPassTransmissiveDrawFunction.intern(),
        //         draw_transmissive_pbr,
        //     ),
        //     (
        //         MainPassTransparentDrawFunction.intern(),
        //         draw_transparent_pbr,
        //     ),
        //     (PrepassOpaqueDrawFunction.intern(), draw_opaque_prepass),
        //     (
        //         PrepassAlphaMaskDrawFunction.intern(),
        //         draw_alpha_mask_prepass,
        //     ),
        //     (DeferredOpaqueDrawFunction.intern(), draw_opaque_deferred),
        //     (
        //         DeferredAlphaMaskDrawFunction.intern(),
        //         draw_alpha_mask_deferred,
        //     ),
        //     (ShadowsDrawFunction.intern(), draw_shadows),
        // ]);

        // let render_method = match material.opaque_render_method() {
        //     OpaqueRendererMethod::Forward => OpaqueRendererMethod::Forward,
        //     OpaqueRendererMethod::Deferred => OpaqueRendererMethod::Deferred,
        //     OpaqueRendererMethod::Auto => OpaqueRendererMethod::Forward,
        // };

        // let mut mesh_pipeline_key_bits = MeshPipelineKey::empty();
        // mesh_pipeline_key_bits.set(
        //     MeshPipelineKey::READS_VIEW_TRANSMISSION_TEXTURE,
        //     material.reads_view_transmission_texture(),
        // );

        // let reads_view_transmission_texture =
        //     mesh_pipeline_key_bits.contains(MeshPipelineKey::READS_VIEW_TRANSMISSION_TEXTURE);

        // let render_phase_type = match material.alpha_mode() {
        //     AlphaMode::Blend | AlphaMode::Premultiplied | AlphaMode::Add | AlphaMode::Multiply => {
        //         RenderPhaseType::Transparent
        //     }
        //     _ if reads_view_transmission_texture => RenderPhaseType::Transmissive,
        //     AlphaMode::Opaque | AlphaMode::AlphaToCoverage => RenderPhaseType::Opaque,
        //     AlphaMode::Mask(_) => RenderPhaseType::AlphaMask,
        // };

        // let mut shaders = SmallVec::new();
        // let mut add_shader = |label: InternedShaderLabel, shader_ref: ShaderRef| {
        //     let mayber_shader = match shader_ref {
        //         ShaderRef::Default => None,
        //         ShaderRef::Handle(handle) => Some(handle),
        //         ShaderRef::Path(path) => Some(asset_server.load(path)),
        //     };
        //     if let Some(shader) = mayber_shader {
        //         shaders.push((label, shader));
        //     }
        // };
        // add_shader(MaterialVertexShader.intern(), M::vertex_shader());
        // add_shader(MaterialFragmentShader.intern(), M::fragment_shader());
        // add_shader(PrepassVertexShader.intern(), M::prepass_vertex_shader());
        // add_shader(PrepassFragmentShader.intern(), M::prepass_fragment_shader());
        // add_shader(DeferredVertexShader.intern(), M::deferred_vertex_shader());
        // add_shader(
        //     DeferredFragmentShader.intern(),
        //     M::deferred_fragment_shader(),
        // );

        let depth_pass_vertex_shader_handle = match M::vertex_shader(RenderPass::Depth) {
            bevy_shader::ShaderRef::Default => POINTCLOUD_SHADER_HANDLE,
            bevy_shader::ShaderRef::Handle(handle) => handle,
            bevy_shader::ShaderRef::Path(asset_path) => asset_server.load(asset_path),
        };
        let depth_pass_fragment_shader_handle = match M::fragment_shader(RenderPass::Depth) {
            bevy_shader::ShaderRef::Default => POINTCLOUD_SHADER_HANDLE,
            bevy_shader::ShaderRef::Handle(handle) => handle,
            bevy_shader::ShaderRef::Path(asset_path) => asset_server.load(asset_path),
        };
        let depth_shader_defs = M::shader_defs(RenderPass::Depth);

        let attribute_pass_vertex_shader_handle = match M::vertex_shader(RenderPass::Attribute) {
            bevy_shader::ShaderRef::Default => POINTCLOUD_SHADER_HANDLE,
            bevy_shader::ShaderRef::Handle(handle) => handle,
            bevy_shader::ShaderRef::Path(asset_path) => asset_server.load(asset_path),
        };
        let attribute_pass_fragment_shader_handle = match M::fragment_shader(RenderPass::Attribute)
        {
            bevy_shader::ShaderRef::Default => POINTCLOUD_SHADER_HANDLE,
            bevy_shader::ShaderRef::Handle(handle) => handle,
            bevy_shader::ShaderRef::Path(asset_path) => asset_server.load(asset_path),
        };
        let attribute_shader_defs = M::shader_defs(RenderPass::Attribute);

        let normalize_shader_handle = match M::normalize_shader() {
            bevy_shader::ShaderRef::Default => POINTCLOUD_SHADER_HANDLE,
            bevy_shader::ShaderRef::Handle(handle) => handle,
            bevy_shader::ShaderRef::Path(asset_path) => asset_server.load(asset_path),
        };

        let bindless = false; // material_uses_bindless_resources::<M>(render_device);
        let bind_group_data = material.bind_group_data();
        let material_key = ErasedMaterialKey::new(bind_group_data);
        // fn specialize<M: PointCloudMaterial>(
        //     pipeline: &MaterialPipeline,
        //     descriptor: &mut RenderPipelineDescriptor,
        //     mesh_layout: &MeshVertexBufferLayoutRef,
        //     erased_key: ErasedMaterialPipelineKey,
        // ) -> Result<(), SpecializedMeshPipelineError>
        // where
        //     M::Data: Hash + Clone,
        // {
        //     let material_key = erased_key.material_key.to_key();
        //     M::specialize(
        //         pipeline,
        //         descriptor,
        //         mesh_layout,
        //         MaterialPipelineKey {
        //             mesh_key: erased_key.mesh_key,
        //             bind_group_data: material_key,
        //         },
        //     )
        // }

        let material_layout = M::bind_group_layout_descriptor(render_device);
        let actual_material_layout = pipeline_cache.get_bind_group_layout(&material_layout);

        match material.unprepared_bind_group(
            &actual_material_layout,
            render_device,
            material_param,
            false,
        ) {
            Ok(unprepared) => {
                let bind_group_allocator =
                    bind_group_allocators.get_mut(&TypeId::of::<M>()).unwrap();
                // Allocate or update the material.
                let binding = match render_material_bindings.entry(material_id.into()) {
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
                };

                Ok(PreparedPointCloudMaterial {
                    binding,
                    properties: Arc::new(PointCloudMaterialProperties {
                        material_layout: Some(material_layout),
                        depth_pass_vertex_shader_handle,
                        depth_pass_fragment_shader_handle,
                        depth_shader_defs,
                        attribute_pass_vertex_shader_handle,
                        attribute_pass_fragment_shader_handle,
                        attribute_shader_defs,
                        normalize_shader_handle,
                        // TODO add draw functions
                        bindless,
                        specialize: None, // Some(specialize::<M>),
                        material_key,
                    }),
                })
            }

            Err(AsBindGroupError::RetryNextUpdate) => {
                Err(PrepareAssetError::RetryNextUpdate(material))
            }

            Err(AsBindGroupError::CreateBindGroupDirectly) => {
                // This material has opted out of automatic bind group creation
                // and is requesting a fully-custom bind group. Invoke
                // `as_bind_group` as requested, and store the resulting bind
                // group in the slot.
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

                        Ok(PreparedPointCloudMaterial {
                            binding: material_binding_id,
                            properties: Arc::new(PointCloudMaterialProperties {
                                material_layout: Some(material_layout),
                                depth_pass_vertex_shader_handle,
                                depth_pass_fragment_shader_handle,
                                depth_shader_defs,
                                attribute_pass_vertex_shader_handle,
                                attribute_pass_fragment_shader_handle,
                                attribute_shader_defs,
                                normalize_shader_handle,
                                // TODO add draw functions
                                bindless,
                                specialize: None, // Some(specialize::<M>),
                                material_key,
                            }),
                        })
                    }

                    Err(AsBindGroupError::RetryNextUpdate) => {
                        Err(PrepareAssetError::RetryNextUpdate(material))
                    }

                    Err(other) => Err(PrepareAssetError::AsBindGroupError(other)),
                }
            }

            Err(other) => Err(PrepareAssetError::AsBindGroupError(other)),
        }
    }
}
