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
        lifetimeless::{Read, SRes, SResMut},
        Res, ResMut, SystemParamItem,
    },
};
use bevy_mesh::MeshVertexBufferLayoutRef;
use bevy_pbr::{
    ErasedMaterialKey, ErasedMaterialPipelineKey, MaterialBindGroupAllocator,
    MaterialBindGroupAllocators, MaterialBindingId, MaterialPipeline, RenderMaterialBindings,
};
use bevy_platform::collections::hash_map::Entry;
use bevy_reflect::{std_traits::ReflectDefault, Reflect, TypePath};
use bevy_render::{
    extract_component::ExtractComponentPlugin,
    render_resource::{
        AsBindGroup, AsBindGroupError, BindGroupLayoutDescriptor, PipelineCache,
        RenderPipelineDescriptor, SpecializedMeshPipelineError,
    },
    renderer::RenderDevice,
    sync_world::MainEntity,
    Extract, ExtractSchedule, RenderApp, RenderStartup,
};
use bevy_shader::{Shader, ShaderDefVal, ShaderRef};
use derive_more::derive::From;
pub use resources::*;
pub use simple::*;

use crate::{
    point::Point,
    point_cloud::{PointCloud3d, PointCloudGpuMapper, PointCloudGpuMapperPlugin},
    render::POINTCLOUD_SHADER_HANDLE,
    render_asset::{
        ErasedRenderAssetComponent, ErasedRenderAssetComponentPlugin, PrepareAssetComponentError,
    },
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

#[derive(Component, Debug, Deref, DerefMut, Reflect, From)]
#[reflect(Component, Default, Clone, PartialEq)]
pub struct PointCloudMaterial3d<M: PointCloudMaterial, A: PointCloudGpuMapper> {
    #[deref]
    pub material_handle: Handle<M>,
    _phantom: PhantomData<A>,
}

impl<M: PointCloudMaterial, A: PointCloudGpuMapper> From<Handle<M>> for PointCloudMaterial3d<M, A> {
    fn from(material_handle: Handle<M>) -> Self {
        Self {
            material_handle,
            _phantom: Default::default(),
        }
    }
}

impl<M: PointCloudMaterial, A: PointCloudGpuMapper> Default for PointCloudMaterial3d<M, A> {
    fn default() -> Self {
        Self {
            material_handle: Default::default(),
            _phantom: Default::default(),
        }
    }
}

impl<M: PointCloudMaterial, A: PointCloudGpuMapper> Clone for PointCloudMaterial3d<M, A> {
    fn clone(&self) -> Self {
        Self {
            material_handle: self.material_handle.clone(),
            _phantom: self._phantom,
        }
    }
}

impl<M: PointCloudMaterial, A: PointCloudGpuMapper> PartialEq for PointCloudMaterial3d<M, A> {
    fn eq(&self, other: &Self) -> bool {
        self.material_handle == other.material_handle && self._phantom == other._phantom
    }
}

impl<M: PointCloudMaterial, A: PointCloudGpuMapper> Eq for PointCloudMaterial3d<M, A> {}

impl<M: PointCloudMaterial, A: PointCloudGpuMapper> From<PointCloudMaterial3d<M, A>>
    for AssetId<M>
{
    fn from(point_cloud_material_3d: PointCloudMaterial3d<M, A>) -> Self {
        point_cloud_material_3d.id()
    }
}

impl<M: PointCloudMaterial, A: PointCloudGpuMapper> From<&PointCloudMaterial3d<M, A>>
    for AssetId<M>
{
    fn from(point_cloud_material_3d: &PointCloudMaterial3d<M, A>) -> Self {
        point_cloud_material_3d.id()
    }
}

#[derive(TypePath)]
pub struct PointCloudMaterial3dGpuMapper<M: PointCloudMaterial, A: PointCloudGpuMapper>(
    PhantomData<fn() -> (M, A)>,
);

impl<M: PointCloudMaterial, A: PointCloudGpuMapper> PointCloudGpuMapper
    for PointCloudMaterial3dGpuMapper<M, A>
{
    type Point = A::Point;

    type GpuPoint = A::GpuPoint;

    type Param = A::Param;

    fn convert(
        points: Arc<Vec<Self::Point>>,
        param: &mut SystemParamItem<Self::Param>,
    ) -> Result<
        Arc<Vec<Self::GpuPoint>>,
        PrepareAssetComponentError<crate::point_cloud::PointCloud<Self::Point>>,
    > {
        A::convert(points, param)
    }

    fn asset_usage(
        point_cloud: &crate::point_cloud::PointCloud<Self::Point>,
    ) -> bevy_asset::RenderAssetUsages {
        A::asset_usage(point_cloud)
    }

    fn byte_len(point_cloud: &crate::point_cloud::PointCloud<Self::Point>) -> Option<usize> {
        A::byte_len(point_cloud)
    }

    fn prepare_buffer(
        point_cloud: crate::point_cloud::PointCloud<Self::Point>,
        asset_id: AssetId<crate::point_cloud::PointCloud<Self::Point>>,
        render_device: &RenderDevice,
        param: &mut SystemParamItem<Self::Param>,
    ) -> Result<
        bevy_render::render_resource::Buffer,
        PrepareAssetComponentError<crate::point_cloud::PointCloud<Self::Point>>,
    > {
        A::prepare_buffer(point_cloud, asset_id, render_device, param)
    }
}

#[derive(Default)]
pub struct PointCloudMaterialsPlugin;

impl Plugin for PointCloudMaterialsPlugin {
    fn build(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<RenderPointCloudMaterialInstances>();
        }
    }
}

#[allow(clippy::type_complexity)]
pub struct PointCloudMaterialPlugin<M: PointCloudMaterial, A: PointCloudGpuMapper>(
    PhantomData<fn() -> (A, M)>,
);

impl<M: PointCloudMaterial, A: PointCloudGpuMapper> Default for PointCloudMaterialPlugin<M, A> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<M: PointCloudMaterial, A: PointCloudGpuMapper> Plugin for PointCloudMaterialPlugin<M, A>
where
    M::Data: PartialEq + Eq + Hash + Clone,
{
    fn build(&self, app: &mut App) {
        app.init_asset::<M>().add_plugins((
            ExtractComponentPlugin::<PointCloudMaterial3d<M, A>>::default(),
            ErasedRenderAssetComponentPlugin::<PointCloudMaterial3d<M, A>>::default(),
            PointCloudGpuMapperPlugin::<
                PointCloudMaterial3dGpuMapper<M, A>,
                PointCloudMaterial3d<M, A>,
            >::default(),
        ));

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_systems(RenderStartup, add_material_bind_group_allocator::<M>)
                .add_systems(
                    ExtractSchedule,
                    (
                        extract_point_cloud_materials::<M, A>
                            .in_set(PointCloudMaterialExtractionSystems),
                        early_sweep_point_cloud_material_instances::<M, A>
                            .after(PointCloudMaterialExtractionSystems)
                            .before(late_sweep_point_cloud_material_instances::<A::Point>),
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
fn extract_point_cloud_materials<M: PointCloudMaterial, A: PointCloudGpuMapper>(
    mut material_instances: ResMut<RenderPointCloudMaterialInstances>,
    changed_meshes_query: Extract<
        Query<
            (Entity, &ViewVisibility, &PointCloudMaterial3d<M, A>),
            Or<(Changed<ViewVisibility>, Changed<PointCloudMaterial3d<M, A>>)>,
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
                    mapper_type_id: TypeId::of::<A>(),
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
fn early_sweep_point_cloud_material_instances<M: PointCloudMaterial, A: PointCloudGpuMapper>(
    mut material_instances: ResMut<RenderPointCloudMaterialInstances>,
    mut removed_materials_query: Extract<RemovedComponents<PointCloudMaterial3d<M, A>>>,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ErasedPointCloudMaterialKey {
    pub material_key: ErasedMaterialKey,
    pub mapper_type_id: TypeId,
}

impl Default for ErasedPointCloudMaterialKey {
    fn default() -> Self {
        Self {
            material_key: Default::default(),
            mapper_type_id: TypeId::of::<()>(),
        }
    }
}

/// Common [`Material`] properties, calculated for a specific material instance.
#[allow(clippy::type_complexity)]
pub struct PointCloudMaterialProperties {
    // pub render_phase_type: RenderPhaseType,
    pub mapper_type_id: TypeId,
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
    pub material_key: ErasedPointCloudMaterialKey,
}

impl Default for PointCloudMaterialProperties {
    fn default() -> Self {
        Self {
            mapper_type_id: TypeId::of::<()>(),
            material_layout: Default::default(),
            depth_pass_vertex_shader_handle: Default::default(),
            depth_pass_fragment_shader_handle: Default::default(),
            depth_shader_defs: Default::default(),
            attribute_pass_vertex_shader_handle: Default::default(),
            attribute_pass_fragment_shader_handle: Default::default(),
            attribute_shader_defs: Default::default(),
            normalize_shader_handle: Default::default(),
            bindless: Default::default(),
            specialize: Default::default(),
            material_key: Default::default(),
        }
    }
}

/// Data prepared for a [`Material`] instance.
pub struct PreparedPointCloudMaterial {
    pub binding: MaterialBindingId,
    pub properties: Arc<PointCloudMaterialProperties>,
}

#[derive(TypePath)]
pub struct PointCloudMaterialKey;

impl<M: PointCloudMaterial, A: PointCloudGpuMapper> ErasedRenderAssetComponent
    for PointCloudMaterial3d<M, A>
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

    type QueryData = Read<PointCloudMaterial3d<M, A>>;

    type QueryFilter = ();

    type Key = PointCloudMaterialKey;

    fn asset_id(data: bevy_ecs::query::ROQueryItem<Self::QueryData>) -> AssetId<Self::SourceAsset> {
        data.id()
    }

    fn prepare_asset(
        material: Self::SourceAsset,
        material_id: AssetId<Self::SourceAsset>,
        type_id: TypeId,
        (
            render_device,
            pipeline_cache,
            bind_group_allocators,
            render_material_bindings,
            asset_server,
            material_param,
        ): &mut SystemParamItem<Self::Param>,
    ) -> Result<Self::ErasedAsset, PrepareAssetComponentError<Self::SourceAsset>> {
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
        let material_key = ErasedPointCloudMaterialKey {
            material_key: ErasedMaterialKey::new(bind_group_data),
            mapper_type_id: type_id,
        };

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
                        mapper_type_id: type_id,
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
                Err(PrepareAssetComponentError::RetryNextUpdate(material))
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
                                mapper_type_id: type_id,
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
                        Err(PrepareAssetComponentError::RetryNextUpdate(material))
                    }

                    Err(other) => Err(PrepareAssetComponentError::AsBindGroupError(other)),
                }
            }

            Err(other) => Err(PrepareAssetComponentError::AsBindGroupError(other)),
        }
    }
}
