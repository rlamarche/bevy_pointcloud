use bevy::{
    app::{App, Plugin, PostUpdate},
    asset::{
        prelude::AssetChanged, Asset, AssetApp, AssetEventSystems, AssetId, AssetServer, Handle,
        UntypedAssetId,
    },
    camera::visibility::ViewVisibility,
    core_pipeline::{
        core_3d::{
            AlphaMask3d, Opaque3d, Opaque3dBatchSetKey, Opaque3dBinKey, Transparent3d,
            TransparentSortingInfo3d,
        },
        deferred::{AlphaMask3dDeferred, Opaque3dDeferred},
        prepass::{
            AlphaMask3dPrepass, Opaque3dPrepass, OpaqueNoLightmap3dBatchSetKey,
            OpaqueNoLightmap3dBinKey,
        },
    },
    ecs::{
        change_detection::Tick,
        entity::{EntityHashMap, EntityHashSet},
        prelude::*,
        system::{
            lifetimeless::{SRes, SResMut},
            SystemParam, SystemParamItem, SystemState,
        },
    },
    log::prelude::*,
    material::{
        key::{ErasedMaterialKey, ErasedMeshPipelineKey},
        labels::{DrawFunctionLabel, InternedDrawFunctionLabel, InternedShaderLabel, ShaderLabel},
        AlphaMode, OpaqueRendererMethod, RenderPhaseType,
    },
    math::{Affine3, Affine3Ext as _},
    mesh::{mark_3d_meshes_as_changed_if_their_assets_changed, Mesh3d, MeshVertexBufferLayoutRef},
    pbr::{
        alpha_mode_pipeline_key, check_views_lights_need_specialization,
        collect_meshes_for_gpu_building, prepare_lights, set_mesh_motion_vector_flags,
        FallbackBindlessResources, MaterialBindGroupAllocator, MaterialBindGroupAllocators,
        MaterialBindingId, MeshInputUniform, MeshPipelineKey, MeshUniform, RenderMeshInstanceFlags,
        RenderMeshInstances, SetMeshViewBindGroup, SetMeshViewBindingArrayBindGroup, Shadow,
        Transmissive3d, ViewKeyCache,
    },
    platform::{
        collections::{hash_map::Entry, HashMap, HashSet},
        hash::FixedHasher,
    },
    prelude::{Deref, DerefMut},
    render::{
        batching::gpu_preprocessing::BatchedInstanceBuffers,
        camera::{
            clear_dirty_wireframe_specializations, expire_wireframe_specializations_for_views,
            DirtySpecializationSystems, PendingQueues,
        },
        erased_render_asset::{
            ErasedRenderAsset, ErasedRenderAssetPlugin, ErasedRenderAssets, PrepareAssetError,
        },
        mesh::{allocator::MeshAllocator, RenderMesh},
        prelude::*,
        render_asset::{prepare_assets, RenderAssets},
        render_phase::*,
        render_resource::*,
        renderer::{RenderDevice, RenderQueue},
        sync_world::{MainEntity, MainEntityHashMap, RenderEntity},
        texture::FallbackImage,
        view::{
            ExtractedView, Msaa, RenderVisibilityRanges, RenderVisibleEntities, RetainedViewEntity,
        },
        Extract, GpuResourceAppExt, Render, RenderApp, RenderDebugFlags, RenderStartup,
        RenderSystems,
    },
    shader::{Shader, ShaderDefVal, ShaderRef},
    utils::Parallel,
};
use core::{
    any::{Any, TypeId},
    hash::Hash,
    marker::PhantomData,
};
use smallvec::SmallVec;
use std::sync::Arc;

use crate::{
    clear_dirty_specializations, expire_specializations_for_views, queue_shadows,
    specialize_shadows, BinnedRenderPhaseExt, ChildChunkOf, DrawDepthOnlyPrepass,
    DrawPointCloudInstanced, DrawPrepass, ErasedSplatPipelineKey, GlobalVisiblePointCloudChunks,
    MySetItemPipeline, PendingShadowQueues, PointCloud3d, PointCloudChunk3d,
    PointCloudDirtySpecializations, PointCloudMaterial3d, PointCloudPipeline,
    PointCloudPipelineSystems, PrepassPipeline, PrepassPipelinePlugin, PrepassPipelineSpecializer,
    PrepassPlugin, RenderPointCloudChunkInstances, RenderPointCloudInstances, SetMeshBindGroup,
    SetPointCloudUniformGroup, SimplePointCloudMaterial, SpecializedPointCloudPipeline,
    SpecializedPointCloudPipelines, SpecializedShadowMaterialPipelineCache, SplatPipelineKey,
    SplatSettings,
};

pub const MATERIAL_BIND_GROUP_INDEX: usize = 4;

/// Materials are used alongside [`MaterialPlugin`], [`PointCloud3d`], and [`PointCloudMaterial3d`]
/// to spawn entities that are rendered with a specific [`Material`] type. They serve as an easy to
/// use high level way to render [`PointCloud3d`] entities with custom shader logic.
///
/// Materials must implement [`AsBindGroup`] to define how data will be transferred to the GPU and
/// bound in shaders. [`AsBindGroup`] can be derived, which makes generating bindings
/// straightforward. See the [`AsBindGroup`] docs for details.
pub trait Material: Asset + AsBindGroup + Clone + Sized {
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
        pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        splat_layout: &MeshVertexBufferLayoutRef,
        instance_layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
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
        app.add_plugins((PrepassPipelinePlugin, PrepassPlugin::new(self.debug_flags)));
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
                .init_gpu_resource::<SpecializedPointCloudPipelines<MaterialPipelineSpecializer>>()
                // .init_gpu_resource::<LightKeyCache>()
                .init_gpu_resource::<SpecializedShadowMaterialPipelineCache>()
                // .init_resource::<DrawFunctions<Shadow>>()
                .init_resource::<RenderPointCloudMaterialInstances>()
                .allow_ambiguous_resource::<RenderPointCloudMaterialInstances>()
                .init_resource::<MaterialBindGroupAllocators>()
                .allow_ambiguous_resource::<MaterialBindGroupAllocators>()
                .init_gpu_resource::<PendingMeshMaterialQueues>()
                .allow_ambiguous_resource::<PendingMeshMaterialQueues>()
                .init_gpu_resource::<PendingShadowQueues>()
                .allow_ambiguous_resource::<PendingShadowQueues>()
                .add_render_command::<Shadow, DrawPrepass>()
                .add_render_command::<Shadow, DrawDepthOnlyPrepass>()
                .add_render_command::<Transparent3d, DrawMaterial>()
                .add_render_command::<Opaque3d, DrawMaterial>()
                .add_render_command::<AlphaMask3d, DrawMaterial>()
                .add_render_command::<Transmissive3d, DrawMaterial>()
                .add_systems(
                    RenderStartup,
                    init_material_pipeline.after(PointCloudPipelineSystems),
                )
                .add_systems(
                    Render,
                    (
                        specialize_material_meshes
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
                        prepare_material_bind_groups,
                        write_material_bind_group_buffers,
                    )
                        .chain()
                        .in_set(RenderSystems::PrepareBindGroups),
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
        }
    }
}

/// Adds the necessary ECS resources and render logic to enable rendering entities using the given
/// [`Material`] asset type.
pub struct MaterialPlugin<M: Material> {
    /// Debugging flags that can optionally be set when constructing the renderer.
    pub debug_flags: RenderDebugFlags,
    pub _marker: PhantomData<M>,
}

impl<M: Material> Default for MaterialPlugin<M> {
    fn default() -> Self {
        Self {
            debug_flags: RenderDebugFlags::default(),
            _marker: Default::default(),
        }
    }
}

impl<M: Material> Plugin for MaterialPlugin<M>
where
    M::Data: PartialEq + Eq + Hash + Clone,
{
    fn build(&self, app: &mut App) {
        app.init_asset::<M>()
            .register_type::<PointCloudMaterial3d<M>>()
            .init_resource::<EntitiesNeedingSpecialization<M>>()
            .add_plugins((ErasedRenderAssetPlugin::<PointCloudMaterial3d<M>>::default(),))
            .add_systems(
                PostUpdate,
                check_entities_needing_specialization::<M>
                    .after(AssetEventSystems)
                    .after(mark_3d_meshes_as_changed_if_their_assets_changed),
            );

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_systems(RenderStartup, add_material_bind_group_allocator::<M>)
                .add_systems(
                    ExtractSchedule,
                    (
                        extract_mesh_materials::<M>.in_set(MaterialExtractionSystems),
                        early_sweep_material_instances::<M>
                            .after(MaterialExtractionSystems)
                            .before(late_sweep_material_instances),
                        extract_entities_needs_specialization::<M>
                            .in_set(DirtySpecializationSystems::CheckForChanges),
                        extract_entities_that_need_specializations_removed::<M>
                            .in_set(DirtySpecializationSystems::CheckForRemovals),
                    ),
                );
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
    M: Material,
{
    M::bindless_slot_count().is_some_and(|bindless_slot_count| {
        M::bindless_supported(render_device) && bindless_slot_count.resolve() > 1
    })
}

fn add_material_bind_group_allocator<M: Material>(
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

/// A key uniquely identifying a specialized [`MaterialPipeline`].
pub struct MaterialPipelineKey<M: Material> {
    pub mesh_key: MeshPipelineKey,
    pub splat_key: SplatPipelineKey,
    pub bind_group_data: M::Data,
}

/// Render pipeline data for a given [`Material`].
#[derive(Resource, Clone)]
pub struct MaterialPipeline {
    pub pointcloud_pipeline: PointCloudPipeline,
}

pub struct MaterialPipelineSpecializer {
    pub(crate) pipeline: MaterialPipeline,
    pub(crate) properties: Arc<MaterialProperties>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ErasedMaterialPipelineKey {
    pub mesh_key: ErasedMeshPipelineKey,
    pub splat_key: ErasedSplatPipelineKey,
    pub material_key: ErasedMaterialKey,
    pub type_id: TypeId,
}

impl SpecializedPointCloudPipeline for MaterialPipelineSpecializer {
    type Key = ErasedMaterialPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        splat_layout: &MeshVertexBufferLayoutRef,
        instance_layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let concrete_mesh_key: MeshPipelineKey = key.mesh_key.downcast();
        let concrete_splat_key: SplatPipelineKey = key.splat_key.downcast();
        let mut descriptor = self.pipeline.pointcloud_pipeline.specialize(
            (concrete_mesh_key, concrete_splat_key),
            splat_layout,
            instance_layout,
        )?;

        descriptor.vertex.shader_defs.push(ShaderDefVal::UInt(
            "MATERIAL_BIND_GROUP".into(),
            MATERIAL_BIND_GROUP_INDEX as u32,
        ));
        if let Some(ref mut fragment) = descriptor.fragment {
            fragment.shader_defs.push(ShaderDefVal::UInt(
                "MATERIAL_BIND_GROUP".into(),
                MATERIAL_BIND_GROUP_INDEX as u32,
            ));
        };
        if let Some(vertex_shader) = self.properties.get_shader(MaterialVertexShader) {
            descriptor.vertex.shader = vertex_shader.clone();
        }

        if let Some(fragment_shader) = self.properties.get_shader(MaterialFragmentShader) {
            descriptor.fragment.as_mut().unwrap().shader = fragment_shader.clone();
        }

        descriptor.layout.insert(
            MATERIAL_BIND_GROUP_INDEX,
            self.properties.material_layout.as_ref().unwrap().clone(),
        );

        if let Some(specialize) = self.properties.user_specialize {
            specialize(
                &self.pipeline as &dyn Any,
                &mut descriptor,
                splat_layout,
                instance_layout,
                key,
            )?;
        }

        // If bindless mode is on, add a `BINDLESS` define.
        // if self.properties.bindless {
        //     descriptor.vertex.shader_defs.push("BINDLESS".into());
        //     if let Some(ref mut fragment) = descriptor.fragment {
        //         fragment.shader_defs.push("BINDLESS".into());
        //     }
        // }

        Ok(descriptor)
    }
}

pub fn init_material_pipeline(mut commands: Commands, mesh_pipeline: Res<PointCloudPipeline>) {
    commands.insert_resource(MaterialPipeline {
        pointcloud_pipeline: mesh_pipeline.clone(),
    });
}

pub type DrawMaterial = (
    MySetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshViewBindingArrayBindGroup<1>,
    SetMeshBindGroup<2>,
    SetPointCloudUniformGroup<3>,
    SetMaterialBindGroup<MATERIAL_BIND_GROUP_INDEX>,
    DrawPointCloudInstanced,
);

/// Sets the bind group for a given [`Material`] at the configured `I` index.
pub struct SetMaterialBindGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetMaterialBindGroup<I> {
    type Param = (
        SRes<RenderPointCloudChunkInstances>,
        SRes<ErasedRenderAssets<PreparedMaterial>>,
        SRes<RenderPointCloudMaterialInstances>,
        SRes<MaterialBindGroupAllocators>,
    );
    type ViewQuery = ();
    type ItemQuery = ();

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        _item_query: Option<()>,
        (
            render_point_cloud_chunk_instances,
            materials,
            material_instances,
            material_bind_group_allocator,
        ): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let materials = materials.into_inner();
        let material_instances = material_instances.into_inner();
        let material_bind_group_allocators = material_bind_group_allocator.into_inner();

        let Some(chunk_instance) = render_point_cloud_chunk_instances.get(&item.entity()) else {
            warn!("render_point_cloud_chunk_instance missing 4");
            return RenderCommandResult::Skip;
        };

        let Some(material_instance) = material_instances
            .instances
            .get(&chunk_instance.root_entity)
        else {
            info!("missing material 1");
            return RenderCommandResult::Skip;
        };
        let Some(material_bind_group_allocator) =
            material_bind_group_allocators.get(&material_instance.asset_id.type_id())
        else {
            info!("missing material 2");
            return RenderCommandResult::Skip;
        };
        let Some(material) = materials.get(material_instance.asset_id) else {
            info!("missing material 3");

            return RenderCommandResult::Skip;
        };
        // info!("Material: {:?}", material.binding);
        let Some(material_bind_group) = material_bind_group_allocator.get(material.binding.group)
        else {
            info!("missing material 4");

            return RenderCommandResult::Skip;
        };
        let Some(bind_group) = material_bind_group.bind_group() else {
            info!("missing material 5");

            return RenderCommandResult::Skip;
        };
        pass.set_bind_group(I, bind_group, &[]);

        RenderCommandResult::Success
    }
}

/// Stores all extracted instances of all [`Material`]s in the render world.
#[derive(Resource, Default)]
pub struct RenderPointCloudMaterialInstances {
    /// Maps from each entity in the main world to the
    /// [`RenderMaterialInstance`] associated with it.
    pub instances: MainEntityHashMap<RenderMaterialInstance>,
    /// A monotonically-increasing counter, which we use to sweep
    /// [`RenderMaterialInstances::instances`] when the entities and/or required
    /// components are removed.
    pub current_change_tick: Tick,
}

/// A dummy [`AssetId`] that we use as a placeholder whenever a mesh doesn't
/// have a material.
///
/// See the comments in [`RenderMaterialInstances::mesh_material`] for more
/// information.
pub(crate) static DUMMY_POINT_CLOUD_MATERIAL: AssetId<SimplePointCloudMaterial> =
    AssetId::<SimplePointCloudMaterial>::invalid();

impl RenderPointCloudMaterialInstances {
    /// Returns the point cloud material ID for the entity with the given point cloud, or a
    /// dummy mesh material ID if the mesh has no material ID.
    ///
    /// Point clouds almost always have materials, but in very specific circumstances
    /// involving custom pipelines they won't. (See the
    /// `specialized_mesh_pipelines` example.)
    pub(crate) fn point_cloud_material(&self, entity: MainEntity) -> UntypedAssetId {
        match self.instances.get(&entity) {
            Some(render_instance) => render_instance.asset_id,
            None => DUMMY_POINT_CLOUD_MATERIAL.into(),
        }
    }
}

/// The material associated with a single mesh instance in the main world.
///
/// Note that this uses an [`UntypedAssetId`] and isn't generic over the
/// material type, for simplicity.
pub struct RenderMaterialInstance {
    /// The material asset.
    pub asset_id: UntypedAssetId,
    /// The [`RenderMaterialInstances::current_change_tick`] at which this
    /// material instance was last modified.
    pub last_change_tick: Tick,
}

/// A [`SystemSet`] that contains all `extract_mesh_materials` systems.
#[derive(SystemSet, Clone, PartialEq, Eq, Debug, Hash)]
pub struct MaterialExtractionSystems;

/// Fills the [`RenderPointCloudMaterialInstances`] resources from the point clouds in the
/// scene.
fn extract_mesh_materials<M: Material>(
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
                RenderMaterialInstance {
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
/// though an entry will be present in `RemovedComponents<PointCloudMaterial3d<A>>`,
/// we must not remove the entry in `RenderMaterialInstances` which corresponds
/// to material B. To handle this case, we use change ticks to avoid removing
/// the entry if it was updated this frame.
///
/// This is the first of two sweep phases. Because this phase runs once per
/// material type, we need a second phase in order to guarantee that we only
/// bump [`RenderPointCloudMaterialInstances::current_change_tick`] once.
fn early_sweep_material_instances<M>(
    mut material_instances: ResMut<RenderPointCloudMaterialInstances>,
    mut removed_materials_query: Extract<RemovedComponents<PointCloudMaterial3d<M>>>,
) where
    M: Material,
{
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

/// Removes mesh materials from [`RenderPointCloudMaterialInstances`] when their
/// [`ViewVisibility`] components are removed.
///
/// This runs after all invocations of `early_sweep_material_instances` and is
/// responsible for bumping [`RenderPointCloudMaterialInstances::current_change_tick`] in
/// preparation for a new frame.
pub fn late_sweep_material_instances(
    mut material_instances: ResMut<RenderPointCloudMaterialInstances>,
    mut removed_point_clouds_query: Extract<RemovedComponents<PointCloud3d>>,
) {
    let last_change_tick = material_instances.current_change_tick;

    for entity in removed_point_clouds_query.read() {
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

/// Extracts main-world entities requiring pipeline specialization and registers them
/// into the render world's dirty tracking table.
///
/// ### Why it is required
/// In Bevy's rendering architecture, when a point cloud's material flags, layout, or
/// custom attributes change, its associated GPU pipeline may need to be specialized
/// (re-evaluated or recompiled). This system bridges that dirty state between the
/// simulation world and the isolated [`RenderApp`], ensuring the renderer knows which
/// specific entities require pipeline specialization before the draw phase.
pub fn extract_entities_needs_specialization<M>(
    entities_needing_specialization: Extract<Res<EntitiesNeedingSpecialization<M>>>,
    mapper: Extract<Query<&RenderEntity>>,
    mut dirty_specializations: ResMut<PointCloudDirtySpecializations>,
) where
    M: Material,
{
    // Drain the list of entities needing specialization from the main world
    // into the render-world `DirtySpecializations` table.
    for entity in entities_needing_specialization.changed.iter() {
        let Ok(&render_entity) = mapper.get(*entity) else {
            warn!("Render entity for PointCloud3d {} not found in extract_entities_needs_specialization", entity);
            continue;
        };

        dirty_specializations
            .changed_renderables
            .insert(render_entity.entity(), MainEntity::from(*entity));
    }
}

/// A system that adds entities that were judged to need their specializations
/// removed to the appropriate table in [`DirtySpecializations`].
pub fn extract_entities_that_need_specializations_removed<M>(
    entities_needing_specialization: Extract<Res<EntitiesNeedingSpecialization<M>>>,
    mapper: Extract<Query<&RenderEntity>>,
    mut dirty_specializations: ResMut<PointCloudDirtySpecializations>,
) where
    M: Material,
{
    for entity in entities_needing_specialization.removed.iter() {
        let Ok(&render_entity) = mapper.get(*entity) else {
            warn!("Render entity for PointCloud3d {} not found in extract_entities_that_need_specializations_removed", entity);
            continue;
        };

        dirty_specializations
            .removed_renderables
            .insert(render_entity.entity(), MainEntity::from(*entity));
    }
}

/// Temporarily stores entities that were determined to either need their
/// specialized pipelines updated or to have their specialized pipelines
/// removed.
#[derive(Resource, Clone, Debug)]
pub struct EntitiesNeedingSpecialization<M> {
    /// Entities that need to have their pipelines updated.
    pub changed: Vec<Entity>,
    /// Entities that need to have their pipelines removed, *unless* they also
    /// appear in [`Self::changed`].
    ///
    /// We can't determine which entities truly need to have their pipelines removed until all
    pub removed: Vec<Entity>,
    _marker: PhantomData<M>,
}

impl<M> Default for EntitiesNeedingSpecialization<M> {
    fn default() -> Self {
        Self {
            changed: Default::default(),
            removed: Default::default(),
            _marker: Default::default(),
        }
    }
}

/// Stores the [`SpecializedMaterialViewPipelineCache`] for each view.
#[derive(Resource, Deref, DerefMut, Default)]
pub struct SpecializedPointCloudMaterialPipelineCache {
    // view entity -> view pipeline cache
    #[deref]
    map: HashMap<RetainedViewEntity, SpecializedPointCloudMaterialViewPipelineCache>,
}

/// Stores the cached render pipeline ID for each entity in a single view, as
/// well as the last time it was changed.
#[derive(Deref, DerefMut, Default)]
pub struct SpecializedPointCloudMaterialViewPipelineCache {
    // material entity -> (tick, pipeline_id)
    #[deref]
    map: EntityHashMap<CachedRenderPipelineId>,
}

/// Finds 3D entities that have changed in such a way as to potentially require
/// specialization and adds them to the [`EntitiesNeedingSpecialization`] list.
pub fn check_entities_needing_specialization<M>(
    needs_specialization: Query<
        Entity,
        (
            Or<(
                Changed<PointCloud3d>,
                // TODO restore when editing hierarchy don't trigger this
                // AssetChanged<PointCloud3d>,
                Changed<PointCloudMaterial3d<M>>,
                Changed<SplatSettings>,
                AssetChanged<PointCloudMaterial3d<M>>,
            )>,
            With<PointCloudMaterial3d<M>>,
        ),
    >,
    chunks_needing_specialization: Query<
        Entity,
        (
            Or<(
                Changed<PointCloudChunk3d>,
                AssetChanged<PointCloudChunk3d>,
                Changed<Mesh3d>,
                AssetChanged<Mesh3d>,
            )>,
            (With<ChildChunkOf>, With<PointCloudChunk3d>),
        ),
    >,
    global_visible_point_cloud_chunks: Res<GlobalVisiblePointCloudChunks>,
    mut par_local: Local<Parallel<Vec<Entity>>>,
    mut entities_needing_specialization: ResMut<EntitiesNeedingSpecialization<M>>,
    mut removed_point_cloud_3d_components: RemovedComponents<PointCloud3d>,
    mut removed_point_cloud_chunk_3d_components: RemovedComponents<PointCloudChunk3d>,
    mut removed_mesh_material_3d_components: RemovedComponents<PointCloudMaterial3d<M>>,
    // reused hashset to prevent duplicates
    mut deduplicate_entities_hash_set: Local<EntityHashSet>,
) where
    M: Material,
{
    entities_needing_specialization.changed.clear();
    entities_needing_specialization.removed.clear();

    // When a [`PointCloud3d`] or its material changed, we need to re-specialize all it's
    // children.
    needs_specialization.par_iter().for_each(|entity| {
        // When a [`PointCloud3d`] or its material changed, we need to re-specialize all it's
        // children.
        if let Some(chunks) = global_visible_point_cloud_chunks.get(&entity) {
            for chunk_entity in chunks.keys() {
                par_local.borrow_local_mut().push(*chunk_entity);
            }
        }
    });
    for queue in par_local.drain() {
        deduplicate_entities_hash_set.insert(queue);
    }

    // Gather all entities that need their specializations regenerated.
    // TODO: we get also get not visible chunks loaded here, do something to ignore them ?
    chunks_needing_specialization.par_iter().for_each(|entity| {
        par_local.borrow_local_mut().push(entity);
    });
    for queue in par_local.drain() {
        deduplicate_entities_hash_set.insert(queue);
    }

    entities_needing_specialization
        .changed
        .reserve(deduplicate_entities_hash_set.len());
    for entity in deduplicate_entities_hash_set.drain() {
        entities_needing_specialization.changed.push(entity);
    }

    // All entities that removed their `PointCloud3d` or `PointCloudMaterial3d` components
    // need to have their specializations removed as well.
    //
    // It's possible that `PointCloud3d` was removed and re-added in the same frame,
    // but we don't have to handle that situation specially here, because
    // `specialize_material_meshes` processes specialization removals before
    // additions. So, if the pipeline specialization gets spuriously removed,
    // it'll just be immediately re-added again, which is harmless.
    for entity in removed_point_cloud_3d_components
        .read()
        .chain(removed_mesh_material_3d_components.read())
    {
        // TODO is it necessary ? because chunks are treated below
        if let Some(chunks) = global_visible_point_cloud_chunks.get(&entity) {
            for chunk_entity in chunks.keys() {
                deduplicate_entities_hash_set.insert(*chunk_entity);
            }
        }
    }

    // process also individual chunks
    for entity in removed_point_cloud_chunk_3d_components.read() {
        deduplicate_entities_hash_set.insert(entity);
    }

    entities_needing_specialization
        .removed
        .reserve(deduplicate_entities_hash_set.len());
    for entity in deduplicate_entities_hash_set.drain() {
        entities_needing_specialization.removed.push(entity);
    }
}

pub(crate) struct SpecializationWorkItem {
    render_entity: Entity,
    #[expect(unused, reason = "Useful for debugging.")]
    visible_entity: MainEntity,
    retained_view_entity: RetainedViewEntity,
    mesh_key: MeshPipelineKey,
    splat_key: SplatPipelineKey,
    splat_layout: MeshVertexBufferLayoutRef,
    instance_layout: MeshVertexBufferLayoutRef,
    properties: Arc<MaterialProperties>,
    material_type_id: TypeId,
}

/// Holds all entities with mesh materials that couldn't be specialized and/or
/// queued because their materials hadn't loaded yet.
///
/// See the [`PendingQueues`] documentation for more information.
#[derive(Default, Deref, DerefMut, Resource)]
pub struct PendingMeshMaterialQueues(pub PendingQueues);

#[derive(SystemParam)]
pub(crate) struct SpecializeMaterialMeshesSystemParam<'w, 's> {
    render_meshes: Res<'w, RenderAssets<RenderMesh>>,
    render_materials: Res<'w, ErasedRenderAssets<PreparedMaterial>>,
    render_mesh_instances: Res<'w, RenderMeshInstances>,
    render_material_instances: Res<'w, RenderPointCloudMaterialInstances>,
    render_point_cloud_instances: Res<'w, RenderPointCloudInstances>,
    render_point_cloud_chunk_instances: Res<'w, RenderPointCloudChunkInstances>,
    // render_lightmaps: Res<'w, RenderLightmaps>,
    render_visibility_ranges: Res<'w, RenderVisibilityRanges>,
    opaque_render_phases: Res<'w, ViewBinnedRenderPhases<Opaque3d>>,
    alpha_mask_render_phases: Res<'w, ViewBinnedRenderPhases<AlphaMask3d>>,
    transmissive_render_phases: Res<'w, ViewSortedRenderPhases<Transmissive3d>>,
    transparent_render_phases: Res<'w, ViewSortedRenderPhases<Transparent3d>>,
    views: Query<'w, 's, (&'static ExtractedView, &'static RenderVisibleEntities)>,
    view_key_cache: Res<'w, ViewKeyCache>,
    specialized_material_pipeline_cache: ResMut<'w, SpecializedPointCloudMaterialPipelineCache>,
    pending_mesh_material_queues: ResMut<'w, PendingMeshMaterialQueues>,
    dirty_specializations: Res<'w, PointCloudDirtySpecializations>,
}

pub(crate) fn specialize_material_meshes(
    world: &mut World,
    state: &mut SystemState<SpecializeMaterialMeshesSystemParam>,
    mut work_items: Local<Vec<SpecializationWorkItem>>,
    mut all_views: Local<HashSet<RetainedViewEntity, FixedHasher>>,
) {
    work_items.clear();
    all_views.clear();

    {
        let SpecializeMaterialMeshesSystemParam {
            render_meshes,
            render_materials,
            render_mesh_instances,
            render_material_instances,
            render_point_cloud_instances,
            render_point_cloud_chunk_instances,
            // render_lightmaps,
            render_visibility_ranges,
            opaque_render_phases,
            alpha_mask_render_phases,
            transmissive_render_phases,
            transparent_render_phases,
            views,
            view_key_cache,
            mut specialized_material_pipeline_cache,
            mut pending_mesh_material_queues,
            dirty_specializations,
        } = state.get_mut(world).unwrap();

        for (view, visible_entities) in &views {
            all_views.insert(view.retained_view_entity);

            if !transparent_render_phases.contains_key(&view.retained_view_entity)
                && !opaque_render_phases.contains_key(&view.retained_view_entity)
                && !alpha_mask_render_phases.contains_key(&view.retained_view_entity)
                && !transmissive_render_phases.contains_key(&view.retained_view_entity)
            {
                continue;
            }

            let Some(view_key) = view_key_cache.get(&view.retained_view_entity) else {
                continue;
            };

            let Some(visible_entities_class) = visible_entities.get::<PointCloudChunk3d>() else {
                continue;
            };

            let mut maybe_specialized_material_pipeline_cache =
                specialized_material_pipeline_cache.get_mut(&view.retained_view_entity);

            // Remove cached pipeline IDs corresponding to entities that either
            // have been removed or need to be re-specialized.
            if let Some(ref mut specialized_material_pipeline_cache) =
                maybe_specialized_material_pipeline_cache
            {
                if dirty_specializations
                    .must_wipe_specializations_for_view(view.retained_view_entity)
                {
                    specialized_material_pipeline_cache.clear();
                } else {
                    for (&renderable_entity, &_main_entity) in
                        dirty_specializations.iter_to_despecialize()
                    {
                        specialized_material_pipeline_cache.remove(&renderable_entity);
                    }
                }
            }

            // Initialize the pending queues.
            let view_pending_mesh_material_queues =
                pending_mesh_material_queues.prepare_for_new_frame(view.retained_view_entity);

            // Now process all meshes that need to be specialized.
            for (render_entity, visible_entity) in dirty_specializations.iter_to_specialize(
                view.retained_view_entity,
                visible_entities_class,
                &view_pending_mesh_material_queues.prev_frame,
            ) {
                if maybe_specialized_material_pipeline_cache
                    .as_ref()
                    .is_some_and(|specialized_material_pipeline_cache| {
                        specialized_material_pipeline_cache.contains_key(render_entity)
                    })
                {
                    // the specialized material is already in cache, safe to continue
                    continue;
                }

                // our entity is a chunk, we need parent point cloud to get its specialized pipeline
                // & material
                let Some(render_point_cloud_chunk_instance) =
                    render_point_cloud_chunk_instances.get(render_entity)
                else {
                    warn!(
                        "RenderPointCloudChunkInstance not found for entity {:?}",
                        visible_entity
                    );
                    continue;
                };

                let Some(render_point_cloud_instance) = render_point_cloud_instances
                    .get(&render_point_cloud_chunk_instance.root_entity)
                else {
                    warn!(
                        "RenderPointCloudInstance not found for entity {:?}",
                        visible_entity
                    );
                    continue;
                };

                // Check for material instance, mesh, and material. If any of
                // these fail, it's probably because the relevant asset hasn't
                // loaded yet. In that case, add the entity to the list of
                // pending mesh materials and bail.
                let Some(material_instance) = render_material_instances
                    .instances
                    .get(&render_point_cloud_chunk_instance.root_entity)
                else {
                    warn!(
                        "Unable to load material instance for entity {:?}",
                        visible_entity
                    );
                    view_pending_mesh_material_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };

                // get the mesh instance from the root entity
                let Some(mesh_instance) = render_mesh_instances
                    .render_mesh_queue_data(render_point_cloud_chunk_instance.root_entity)
                else {
                    warn!(
                        "mesh_instance not found for entity {:?}",
                        render_point_cloud_chunk_instance.root_entity
                    );
                    view_pending_mesh_material_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };

                // get the mesh from the chunk
                let Some(mesh) = render_meshes.get(render_point_cloud_chunk_instance.mesh_asset_id)
                else {
                    warn!(
                        "render_meshes not found for asset id {:?}",
                        render_point_cloud_chunk_instance.mesh_asset_id
                    );
                    view_pending_mesh_material_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };

                let Some(material) = render_materials.get(material_instance.asset_id) else {
                    warn!("render_materials not found");
                    view_pending_mesh_material_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };

                let Some(splat_mesh) = render_meshes.get(render_point_cloud_instance.splat) else {
                    warn!("shape mesh not found");
                    view_pending_mesh_material_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };

                let mut mesh_pipeline_key_bits: MeshPipelineKey =
                    material.properties.mesh_pipeline_key_bits.downcast();
                mesh_pipeline_key_bits.insert(alpha_mode_pipeline_key(
                    material.properties.alpha_mode,
                    &Msaa::from_samples(view_key.msaa_samples()),
                ));
                let mut mesh_key = *view_key
                    | MeshPipelineKey::from_bits_retain(mesh.key_bits.bits())
                    | mesh_pipeline_key_bits;

                // if let Some(lightmap) = render_lightmaps.render_lightmaps.get(visible_entity) {
                //     mesh_key |= MeshPipelineKey::LIGHTMAPPED;

                //     if lightmap.bicubic_sampling {
                //         mesh_key |= MeshPipelineKey::LIGHTMAP_BICUBIC_SAMPLING;
                //     }
                // }

                if render_visibility_ranges
                    .entity_has_crossfading_visibility_ranges(*visible_entity)
                {
                    mesh_key |= MeshPipelineKey::VISIBILITY_RANGE_DITHER;
                }

                if view_key.contains(MeshPipelineKey::MOTION_VECTOR_PREPASS) {
                    if mesh_instance
                        .flags()
                        .contains(RenderMeshInstanceFlags::HAS_PREVIOUS_SKIN)
                    {
                        mesh_key |= MeshPipelineKey::HAS_PREVIOUS_SKIN;
                    }
                    if mesh_instance
                        .flags()
                        .contains(RenderMeshInstanceFlags::HAS_PREVIOUS_MORPH)
                    {
                        mesh_key |= MeshPipelineKey::HAS_PREVIOUS_MORPH;
                    }
                }

                work_items.push(SpecializationWorkItem {
                    // this point to a PointCloud3d
                    render_entity: *render_entity,
                    visible_entity: *visible_entity,
                    retained_view_entity: view.retained_view_entity,
                    mesh_key,
                    splat_key: (&render_point_cloud_instance.splat_settings).into(),
                    splat_layout: splat_mesh.layout.clone(),
                    instance_layout: mesh.layout.clone(),
                    properties: material.properties.clone(),
                    material_type_id: material_instance.asset_id.type_id(),
                });
            }
        }

        pending_mesh_material_queues.expire_stale_views(&all_views);
    }

    for item in work_items.drain(..) {
        let key = ErasedMaterialPipelineKey {
            type_id: item.material_type_id,
            mesh_key: ErasedMeshPipelineKey::new(item.mesh_key),
            splat_key: ErasedSplatPipelineKey::new(item.splat_key),
            material_key: item.properties.material_key.clone(),
        };

        let Some(base_specialize) = item.properties.base_specialize else {
            warn!("no base specialize");
            continue;
        };
        match base_specialize(
            world,
            key,
            &item.splat_layout,
            &item.instance_layout,
            &item.properties,
        ) {
            Ok(pipeline_id) => {
                world
                    .resource_mut::<SpecializedPointCloudMaterialPipelineCache>()
                    .entry(item.retained_view_entity)
                    .or_default()
                    .insert(item.render_entity, pipeline_id);
            }
            Err(err) => error!("{}", err),
        }
    }

    world
        .resource_mut::<SpecializedPointCloudMaterialPipelineCache>()
        .retain(|view, _| all_views.contains(view));
}

/// For each view, iterates over all the meshes visible from that view and adds
/// them to [`BinnedRenderPhase`]s or [`SortedRenderPhase`]s as appropriate.
pub fn queue_material_meshes(
    render_materials: Res<ErasedRenderAssets<PreparedMaterial>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    render_material_instances: Res<RenderPointCloudMaterialInstances>,
    render_point_cloud_chunk_instances: Res<RenderPointCloudChunkInstances>,
    mesh_assets: Res<RenderAssets<RenderMesh>>,
    mesh_allocator: Res<MeshAllocator>,
    maybe_batched_instance_buffers: Option<
        Res<BatchedInstanceBuffers<MeshUniform, MeshInputUniform>>,
    >,
    mut opaque_render_phases: ResMut<ViewBinnedRenderPhases<Opaque3d>>,
    mut alpha_mask_render_phases: ResMut<ViewBinnedRenderPhases<AlphaMask3d>>,
    mut transmissive_render_phases: ResMut<ViewSortedRenderPhases<Transmissive3d>>,
    mut transparent_render_phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    mut pending_mesh_material_queues: ResMut<PendingMeshMaterialQueues>,
    views: Query<(&ExtractedView, &RenderVisibleEntities)>,
    // don't know why it is `ResMut` here, but I suspect it is to have exclusive read access
    specialized_material_pipeline_cache: ResMut<SpecializedPointCloudMaterialPipelineCache>,
    dirty_specializations: Res<PointCloudDirtySpecializations>,
) {
    for (view, visible_entities) in &views {
        let (
            Some(opaque_phase),
            Some(alpha_mask_phase),
            Some(transmissive_phase),
            Some(transparent_phase),
        ) = (
            opaque_render_phases.get_mut(&view.retained_view_entity),
            alpha_mask_render_phases.get_mut(&view.retained_view_entity),
            transmissive_render_phases.get_mut(&view.retained_view_entity),
            transparent_render_phases.get_mut(&view.retained_view_entity),
        )
        else {
            continue;
        };

        // info!("opaque phase items count: {}", opaque_phase.count_items());

        let Some(view_specialized_material_pipeline_cache) =
            specialized_material_pipeline_cache.get(&view.retained_view_entity)
        else {
            // warn!("no view_specialized_material_pipeline_cache for view");
            continue;
        };

        let Some(visible_entities_class) = visible_entities.get::<PointCloudChunk3d>() else {
            continue;
        };

        // First, remove meshes that need to be respecialized, and those that were removed, from the
        // bins.
        for (render_entity, main_entity) in
            dirty_specializations.iter_to_dequeue(view.retained_view_entity, visible_entities_class)
        {
            let Some(render_point_cloud_chunk_instance) =
                render_point_cloud_chunk_instances.get(render_entity)
            else {
                warn!(
                    "RenderPointCloudChunkInstance not found for entity {:?} when removing phase",
                    main_entity
                );
                continue;
            };

            opaque_phase.remove_unbatchable_entity_pair(
                render_entity,
                &render_point_cloud_chunk_instance.root_entity,
            );
            alpha_mask_phase.remove_unbatchable_entity_pair(
                render_entity,
                &render_point_cloud_chunk_instance.root_entity,
            );
            transmissive_phase.remove(
                *render_entity,
                render_point_cloud_chunk_instance.root_entity,
            );
            transparent_phase.remove(
                *render_entity,
                render_point_cloud_chunk_instance.root_entity,
            );
        }

        // Fetch the pending mesh material queues for this view.
        let view_pending_mesh_material_queues = pending_mesh_material_queues
            .get_mut(&view.retained_view_entity)
            .expect(
                "View pending mesh material queues should have been created in \
                 `specialize_material_meshes`",
            );

        // Now iterate through all newly-visible entities and those needing respecialization.
        for (render_entity, visible_entity) in dirty_specializations.iter_to_queue(
            view.retained_view_entity,
            visible_entities_class,
            &view_pending_mesh_material_queues.prev_frame,
        ) {
            let Some(pipeline_id) = view_specialized_material_pipeline_cache
                .get(render_entity)
                .copied()
            else {
                warn!("view_specialized_material_pipeline_cache missing");
                continue;
            };

            // our entity is a chunk, we need parent point cloud to get its specialized pipeline
            // & material
            let Some(render_point_cloud_chunk_instance) =
                render_point_cloud_chunk_instances.get(render_entity)
            else {
                warn!(
                    "RenderPointCloudChunkInstance not found for entity {:?}",
                    visible_entity
                );
                continue;
            };

            // Check for material instance, mesh, and material. If any of these
            // fail, it's probably because the relevant asset hasn't loaded yet.
            // In that case, add the entity to the list of pending mesh
            // materials and bail.
            let Some(material_instance) = render_material_instances
                .instances
                .get(&render_point_cloud_chunk_instance.root_entity)
            else {
                warn!("material instance not ready, queue");
                view_pending_mesh_material_queues
                    .current_frame
                    .insert((*render_entity, *visible_entity));
                continue;
            };
            // get the mesh instance from the root entity
            let Some(mesh_instance) = render_mesh_instances
                .render_mesh_queue_data(render_point_cloud_chunk_instance.root_entity)
            else {
                warn!("mesh instance not ready, queue");
                view_pending_mesh_material_queues
                    .current_frame
                    .insert((*render_entity, *visible_entity));
                continue;
            };
            let Some(material) = render_materials.get(material_instance.asset_id) else {
                warn!("material not ready, queue");
                view_pending_mesh_material_queues
                    .current_frame
                    .insert((*render_entity, *visible_entity));
                continue;
            };

            // Fetch the slabs that this mesh resides in.
            let Some(mesh_slabs) =
                mesh_allocator.mesh_slabs(&render_point_cloud_chunk_instance.mesh_asset_id)
            else {
                warn!(
                    "mesh slab not found for visible entity {:?}",
                    visible_entity
                );
                view_pending_mesh_material_queues
                    .current_frame
                    .insert((*render_entity, *visible_entity));
                continue;
            };

            match material.properties.render_phase_type {
                RenderPhaseType::Transmissive => {
                    let Some(draw_function) = material
                        .properties
                        .get_draw_function(MainPassTransmissiveDrawFunction)
                    else {
                        continue;
                    };
                    transmissive_phase.add_retained(Transmissive3d {
                        sorting_info: TransparentSortingInfo3d::Sorted {
                            mesh_center: get_mesh_instance_world_from_local(
                                *visible_entity,
                                mesh_instance.current_uniform_index,
                                &render_mesh_instances,
                                maybe_batched_instance_buffers.as_deref(),
                            )
                            .transform_point3(
                                mesh_assets
                                    .get(mesh_instance.mesh_asset_id())
                                    .unwrap()
                                    .aabb_center,
                            ),
                            depth_bias: material.properties.depth_bias,
                        },
                        entity: (
                            // Bevy sends PLACEHOLDER here, why ?
                            *render_entity,
                            // use the root entity here to correctly handle `RenderMeshInstances`
                            // in binned render phases
                            render_point_cloud_chunk_instance.root_entity,
                        ),
                        draw_function,
                        pipeline: pipeline_id,
                        batch_range: 0..1,
                        extra_index: PhaseItemExtraIndex::None,
                        indexed: mesh_slabs.index_slab_id.is_some(),
                        // Filled in later.
                        distance: 0.0,
                    });
                }
                RenderPhaseType::Opaque => {
                    if material.properties.render_method == OpaqueRendererMethod::Deferred {
                        // Even though we aren't going to insert the entity into
                        // a bin, we still want to update its cache entry. That
                        // way, we know we don't need to re-examine it in future
                        // frames.
                        opaque_phase.update_cache(*visible_entity, None);
                        warn!("DEFERRED");
                        continue;
                    }
                    let Some(draw_function) = material
                        .properties
                        .get_draw_function(MainPassOpaqueDrawFunction)
                    else {
                        warn!("draw function not found");
                        continue;
                    };
                    let batch_set_key = Opaque3dBatchSetKey {
                        pipeline: pipeline_id,
                        draw_function,
                        material_bind_group_index: Some(material.binding.group.0),
                        slabs: mesh_slabs,
                        lightmap_slab: mesh_instance
                            .shared
                            .lightmap_slab_index()
                            .map(|index| *index),
                    };
                    let bin_key = Opaque3dBinKey {
                        asset_id: render_point_cloud_chunk_instance.mesh_asset_id.into(),
                    };

                    opaque_phase.add(
                        batch_set_key,
                        bin_key,
                        (
                            *render_entity,
                            // use the root entity here to correctly handle
                            // [`GetFullBatchData::get_binned_index`]
                            // in binned render phases
                            render_point_cloud_chunk_instance.root_entity,
                        ),
                        mesh_instance.current_uniform_index,
                        BinnedRenderPhaseType::UnbatchableMesh,
                    );
                }
                // Alpha mask
                RenderPhaseType::AlphaMask => {
                    let Some(draw_function) = material
                        .properties
                        .get_draw_function(MainPassAlphaMaskDrawFunction)
                    else {
                        info!("missing drawfunction for MainPassAlphaMaskDrawFunction");
                        continue;
                    };
                    let batch_set_key = OpaqueNoLightmap3dBatchSetKey {
                        draw_function,
                        pipeline: pipeline_id,
                        material_bind_group_index: Some(material.binding.group.0),
                        slabs: mesh_slabs,
                    };
                    let bin_key = OpaqueNoLightmap3dBinKey {
                        asset_id: mesh_instance.mesh_asset_id().into(),
                    };
                    alpha_mask_phase.add(
                        batch_set_key,
                        bin_key,
                        (
                            *render_entity,
                            // use the root entity here to correctly handle
                            // [`GetFullBatchData::get_binned_index`]
                            // in binned render phases
                            render_point_cloud_chunk_instance.root_entity,
                        ),
                        mesh_instance.current_uniform_index,
                        BinnedRenderPhaseType::UnbatchableMesh,
                    );
                }
                RenderPhaseType::Transparent => {
                    let Some(draw_function) = material
                        .properties
                        .get_draw_function(MainPassTransparentDrawFunction)
                    else {
                        continue;
                    };
                    transparent_phase.add_retained(Transparent3d {
                        sorting_info: TransparentSortingInfo3d::Sorted {
                            mesh_center: get_mesh_instance_world_from_local(
                                *visible_entity,
                                mesh_instance.current_uniform_index,
                                &render_mesh_instances,
                                maybe_batched_instance_buffers.as_deref(),
                            )
                            .transform_point3(
                                mesh_assets
                                    .get(mesh_instance.mesh_asset_id())
                                    .unwrap()
                                    .aabb_center,
                            ),
                            depth_bias: material.properties.depth_bias,
                        },
                        entity: (
                            // Bevy sends PLACEHOLDER here, why ?
                            *render_entity,
                            // use the root entity here to correctly handle
                            // [`GetFullBatchData::get_binned_index`]
                            // in binned render phases
                            render_point_cloud_chunk_instance.root_entity,
                        ),
                        draw_function,
                        pipeline: pipeline_id,
                        batch_range: 0..1,
                        extra_index: PhaseItemExtraIndex::None,
                        indexed: mesh_slabs.index_slab_id.is_some(),
                        // Filled in later.
                        distance: 0.0,
                    });
                }
            }
        }
    }
}

// /// Default render method used for opaque materials.
// #[derive(Default, Resource, Clone, Debug, ExtractResource, Reflect)]
// #[reflect(Resource, Default, Debug, Clone)]
// pub struct DefaultOpaqueRendererMethod(OpaqueRendererMethod);

// impl DefaultOpaqueRendererMethod {
//     pub fn forward() -> Self {
//         DefaultOpaqueRendererMethod(OpaqueRendererMethod::Forward)
//     }

//     pub fn deferred() -> Self {
//         DefaultOpaqueRendererMethod(OpaqueRendererMethod::Deferred)
//     }

//     pub fn set_to_forward(&mut self) {
//         self.0 = OpaqueRendererMethod::Forward;
//     }

//     pub fn set_to_deferred(&mut self) {
//         self.0 = OpaqueRendererMethod::Deferred;
//     }
// }

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

/// A resource that maps each untyped material ID to its binding.
///
/// This duplicates information in `RenderAssets<M>`, but it doesn't have the
/// `M` type parameter, so it can be used in untyped contexts like
/// [`crate::render::mesh::collect_meshes_for_gpu_building`].
#[derive(Resource, Default, Deref, DerefMut)]
pub struct RenderMaterialBindings(HashMap<UntypedAssetId, MaterialBindingId>);

/// Data prepared for a [`Material`] instance.
pub struct PreparedMaterial {
    pub binding: MaterialBindingId,
    pub properties: Arc<MaterialProperties>,
}

pub fn base_specialize(
    world: &mut World,
    key: ErasedMaterialPipelineKey,
    splat_layout: &MeshVertexBufferLayoutRef,
    instance_layout: &MeshVertexBufferLayoutRef,
    properties: &Arc<MaterialProperties>,
) -> Result<CachedRenderPipelineId, SpecializedMeshPipelineError> {
    world.resource_scope(
        |world, mut pipelines: Mut<SpecializedPointCloudPipelines<MaterialPipelineSpecializer>>| {
            let mesh_pipeline = world.resource::<PointCloudPipeline>().clone();
            let pipeline_cache = world.resource::<PipelineCache>();

            let specializer = MaterialPipelineSpecializer {
                pipeline: MaterialPipeline {
                    pointcloud_pipeline: mesh_pipeline,
                },
                properties: properties.clone(),
            };

            pipelines.specialize(
                pipeline_cache,
                &specializer,
                key,
                splat_layout,
                instance_layout,
            )
        },
    )
}

fn prepass_specialize(
    world: &mut World,
    key: ErasedMaterialPipelineKey,
    splat_layout: &MeshVertexBufferLayoutRef,
    instance_layout: &MeshVertexBufferLayoutRef,
    properties: &Arc<MaterialProperties>,
) -> Result<CachedRenderPipelineId, SpecializedMeshPipelineError> {
    world.resource_scope(
        |world, mut pipelines: Mut<SpecializedPointCloudPipelines<PrepassPipelineSpecializer>>| {
            let prepass_pipeline = world.resource::<PrepassPipeline>().clone();
            let pipeline_cache = world.resource::<PipelineCache>();

            let specializer = PrepassPipelineSpecializer {
                pipeline: prepass_pipeline,
                properties: properties.clone(),
            };

            pipelines.specialize(
                pipeline_cache,
                &specializer,
                key,
                splat_layout,
                instance_layout,
            )
        },
    )
}

fn user_specialize<M: Material>(
    pipeline: &dyn Any,
    descriptor: &mut RenderPipelineDescriptor,
    splat_layout: &MeshVertexBufferLayoutRef,
    instance_layout: &MeshVertexBufferLayoutRef,
    erased_key: ErasedMaterialPipelineKey,
) -> Result<(), SpecializedMeshPipelineError>
where
    M::Data: Hash + Clone,
{
    let pipeline = pipeline.downcast_ref::<MaterialPipeline>().unwrap();
    let material_key = erased_key.material_key.to_key();
    let mesh_key: MeshPipelineKey = erased_key.mesh_key.downcast();
    let splat_key: SplatPipelineKey = erased_key.splat_key.downcast();
    M::specialize(
        pipeline,
        descriptor,
        splat_layout,
        instance_layout,
        MaterialPipelineKey {
            mesh_key,
            splat_key,
            bind_group_data: material_key,
        },
    )
}

// orphan rules T_T
impl<M: Material> ErasedRenderAsset for PointCloudMaterial3d<M>
where
    M::Data: PartialEq + Eq + Hash + Clone,
{
    type SourceAsset = M;
    type ErasedAsset = PreparedMaterial;

    type Param = (
        SRes<RenderDevice>,
        SRes<PipelineCache>,
        // SRes<DefaultOpaqueRendererMethod>,
        SResMut<MaterialBindGroupAllocators>,
        SResMut<RenderMaterialBindings>,
        SRes<DrawFunctions<Opaque3d>>,
        SRes<DrawFunctions<AlphaMask3d>>,
        SRes<DrawFunctions<Transmissive3d>>,
        SRes<DrawFunctions<Transparent3d>>,
        SRes<DrawFunctions<Opaque3dPrepass>>,
        SRes<DrawFunctions<AlphaMask3dPrepass>>,
        SRes<DrawFunctions<Opaque3dDeferred>>,
        SRes<DrawFunctions<AlphaMask3dDeferred>>,
        SRes<DrawFunctions<Shadow>>,
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
            opaque_draw_functions,
            alpha_mask_draw_functions,
            transmissive_draw_functions,
            transparent_draw_functions,
            opaque_prepass_draw_functions,
            alpha_mask_prepass_draw_functions,
            opaque_deferred_draw_functions,
            alpha_mask_deferred_draw_functions,
            shadow_draw_functions,
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

        let draw_opaque_pbr = opaque_draw_functions.read().id::<DrawMaterial>();
        let draw_alpha_mask_pbr = alpha_mask_draw_functions.read().id::<DrawMaterial>();
        let draw_transmissive_pbr = transmissive_draw_functions.read().id::<DrawMaterial>();
        let draw_transparent_pbr = transparent_draw_functions.read().id::<DrawMaterial>();
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

        Ok(PreparedMaterial {
            binding,
            properties: Arc::new(MaterialProperties {
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
            }),
        })
    }

    fn unload_asset(
        source_asset: AssetId<Self::SourceAsset>,
        (_, _, /* _, */ bind_group_allocators, render_material_bindings, ..): &mut SystemParamItem<
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

/// Creates and/or recreates any bind groups that contain materials that were
/// modified this frame.
pub fn prepare_material_bind_groups(
    mut allocators: ResMut<MaterialBindGroupAllocators>,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    fallback_image: Res<FallbackImage>,
    fallback_resources: Res<FallbackBindlessResources>,
) {
    for (_, allocator) in allocators.iter_mut() {
        allocator.prepare_bind_groups(
            &render_device,
            &pipeline_cache,
            &fallback_resources,
            &fallback_image,
        );
    }
}

/// Uploads the contents of all buffers that the [`MaterialBindGroupAllocator`]
/// manages to the GPU.
///
/// Non-bindless allocators don't currently manage any buffers, so this method
/// only has an effect for bindless allocators.
pub fn write_material_bind_group_buffers(
    mut allocators: ResMut<MaterialBindGroupAllocators>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    for (_, allocator) in allocators.iter_mut() {
        allocator.write_buffers(&render_device, &render_queue);
    }
}

/// Returns the world-from-local transform for the given mesh instance.
pub fn get_mesh_instance_world_from_local(
    entity: MainEntity,
    current_uniform_index: InputUniformIndex,
    render_mesh_instances: &RenderMeshInstances,
    maybe_batched_instance_buffers: Option<&BatchedInstanceBuffers<MeshUniform, MeshInputUniform>>,
) -> Affine3 {
    // The way we fetch the world-from-local transform depends on whether we're
    // doing CPU or GPU preprocessing. If we're doing CPU preprocessing, we have
    // the world-from-local transform handy in `RenderMeshInstancesCpu`.
    // Otherwise, if we're doing GPU preprocessing, we need to pull the
    // transform out of the `MeshInputUniform` GPU buffer.
    match *render_mesh_instances {
        RenderMeshInstances::CpuBuilding(ref render_mesh_instances_cpu) => {
            let Some(render_mesh_instance) = render_mesh_instances_cpu.get(&entity) else {
                return Affine3::IDENTITY;
            };
            render_mesh_instance.transforms.world_from_local
        }
        RenderMeshInstances::GpuBuilding(_) => {
            let Some(batched_instance_buffers) = maybe_batched_instance_buffers else {
                return Affine3::IDENTITY;
            };
            let Some(mesh_input_uniform) = batched_instance_buffers
                .current_input_buffer
                .get(current_uniform_index.0)
            else {
                return Affine3::IDENTITY;
            };
            Affine3::from_transpose(mesh_input_uniform.world_from_local)
        }
    }
}

/// Common material properties, calculated for a specific material instance.
#[derive(Default)]
pub struct MaterialProperties {
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
}

impl MaterialProperties {
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

/// A type erased function pointer for specializing a material pipeline. The implementation is
/// expected to:
/// - Look up the appropriate specializer from the world
/// - Downcast the erased key to the concrete key type
/// - Call `SpecializedMeshPipelines::specialize` with the specializer and return the resulting
///   pipeline id
pub type BaseSpecializeFn = fn(
    &mut World,
    ErasedMaterialPipelineKey,
    &MeshVertexBufferLayoutRef,
    &MeshVertexBufferLayoutRef,
    &Arc<MaterialProperties>,
) -> Result<CachedRenderPipelineId, SpecializedMeshPipelineError>;

/// A type erased function pointer for specializing a material prepass pipeline. The implementation
/// is expected to:
/// - Look up the appropriate specializer from the world
/// - Downcast the erased key to the concrete key type
/// - Call `SpecializedMeshPipelines::specialize` with the specializer and return the resulting
///   pipeline id
pub type PrepassSpecializeFn = fn(
    &mut World,
    ErasedMaterialPipelineKey,
    &MeshVertexBufferLayoutRef,
    &MeshVertexBufferLayoutRef,
    &Arc<MaterialProperties>,
) -> Result<CachedRenderPipelineId, SpecializedMeshPipelineError>;

/// A type erased function pointer for specializing a material prepass pipeline. The implementation
/// is expected to:
/// - Look up the appropriate specializer from the world
/// - Downcast the erased key to the concrete key type
/// - Call `SpecializedMeshPipelines::specialize` with the specializer and return the resulting
///   pipeline id
pub type UserSpecializeFn = fn(
    &dyn Any,
    &mut RenderPipelineDescriptor,
    &MeshVertexBufferLayoutRef,
    &MeshVertexBufferLayoutRef,
    ErasedMaterialPipelineKey,
) -> Result<(), SpecializedMeshPipelineError>;
