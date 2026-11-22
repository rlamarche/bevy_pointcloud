use std::{any::TypeId, fmt::Debug, marker::PhantomData};

use bevy::{
    asset::{Asset, AssetId, AssetServer, Handle, UntypedAssetId},
    ecs::{
        change_detection::Tick,
        entity::{Entity, EntityHashMap},
        resource::Resource,
        system::SystemParamItem,
        world::World,
    },
    material::{
        descriptor::{BindGroupLayoutDescriptor, CachedRenderPipelineId},
        labels::{InternedShaderLabel, ShaderLabel},
    },
    platform::collections::{hash_map::Entry, HashMap},
    prelude::{Deref, DerefMut},
    render::{
        camera::PendingQueues,
        erased_render_asset::PrepareAssetError,
        material_bind_groups::{FallbackBuffer, MaterialBindGroupAllocators, MaterialBindingId},
        render_asset::RenderAssets,
        render_resource::{AsBindGroup, AsBindGroupError, BindGroupBuilder, PipelineCache},
        renderer::RenderDevice,
        storage::GpuShaderBuffer,
        sync_world::{MainEntity, MainEntityHashMap, RenderEntity},
        view::RetainedViewEntity,
    },
    shader::{Shader, ShaderRef},
};
use smallvec::SmallVec;

use crate::render::{
    DeferredFragmentShader, DeferredVertexShader, MaterialFragmentShader, MaterialVertexShader,
    PointCloudMaterial, PrepassFragmentShader, PrepassVertexShader,
};

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

impl RenderPointCloudMaterialInstances {
    /// Returns the point cloud material ID for the entity with the given mesh, or
    /// `None` if the mesh has no material ID.
    ///
    /// Meshes almost always have materials, but in very specific circumstances
    /// involving custom pipelines they won't. (See the
    /// `specialized_mesh_pipelines` example.)
    pub(crate) fn point_cloud_material(&self, entity: MainEntity) -> Option<UntypedAssetId> {
        self.instances
            .get(&entity)
            .map(|instance| instance.asset_id)
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

/// Temporarily stores entities that were determined to either need their
/// specialized pipelines updated or to have their specialized pipelines
/// removed.
#[derive(Resource, Clone)]
pub struct EntitiesNeedingSpecialization<M> {
    /// Entities that need to have their pipelines updated.
    pub changed: Vec<(Entity, RenderEntity)>,
    /// Entities that need to have their pipelines removed, *unless* they also
    /// appear in [`Self::changed`].
    ///
    /// We can't determine which entities truly need to have their pipelines removed until all.
    /// Also stores the associated `RenderEntity` because we need it. Populated in observer
    /// [`on_remove_point_cloud_chunk_3d`].
    pub removed: EntityHashMap<RenderEntity>,
    _marker: PhantomData<M>,
}

impl<M> Debug for EntitiesNeedingSpecialization<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EntitiesNeedingSpecialization")
            .field("changed", &self.changed)
            .field("removed", &self.removed)
            .finish()
    }
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

/// Holds all entities with mesh materials that couldn't be specialized and/or
/// queued because their materials hadn't loaded yet.
///
/// See the [`PendingQueues`] documentation for more information.
#[derive(Default, Deref, DerefMut, Resource)]
pub struct PendingPointCloudMaterialQueues(pub PendingQueues);

/// A resource that maps each untyped material ID to its binding.
///
/// This duplicates information in `RenderAssets<M>`, but it doesn't have the
/// `M` type parameter, so it can be used in untyped contexts like
/// `collect_meshes_for_gpu_building`.
#[derive(Resource, Default, Deref, DerefMut)]
pub struct RenderMaterialBindings {
    /// The mapping from each asset ID to its location within the material bind
    /// group allocator.
    #[deref]
    bindings: HashMap<UntypedAssetId, MaterialBindingId>,

    /// The bind group we're building up.
    ///
    /// This is cleared and reused for every asset that we build in order to
    /// reuse allocations.
    bind_group_builder: BindGroupBuilder,
}

impl RenderMaterialBindings {
    /// Prepares a material asset for the render world.
    ///
    /// This method allocates the material in the bind group allocators and
    /// returns its [`MaterialBindingId`].
    pub fn prepare_material<M>(
        &mut self,
        material: &M,
        material_id: AssetId<M>,
        material_param: &mut SystemParamItem<'_, '_, M::Param>,
        material_layout: &BindGroupLayoutDescriptor,
        bind_group_allocators: &mut MaterialBindGroupAllocators,
        render_device: &RenderDevice,
        pipeline_cache: &PipelineCache,
        fallback_buffer: &FallbackBuffer,
        shader_buffer_assets: &RenderAssets<GpuShaderBuffer>,
    ) -> Result<MaterialBindingId, PrepareAssetError<M>>
    where
        M: AsBindGroup + Asset + Clone,
    {
        let actual_material_layout = pipeline_cache.get_bind_group_layout(material_layout);

        // Ask the material to build the bind group.
        let result = match material.build_bind_group(
            &actual_material_layout,
            render_device,
            material_param,
            false,
            &mut self.bind_group_builder,
        ) {
            Ok(()) => {
                let bind_group_allocator =
                    bind_group_allocators.get_mut(&TypeId::of::<M>()).unwrap();
                // Allocate or update the material.
                match self.bindings.entry(material_id.into()) {
                    Entry::Occupied(mut occupied_entry) => {
                        // First, try a fast path. If none of the GPU resource
                        // bindings have changed, then we can just update the
                        // POD and don't have to update any of the bindings in
                        // the bind group allocator.
                        let old_binding = *occupied_entry.get();
                        if bind_group_allocator
                            .try_update_data(old_binding, &mut self.bind_group_builder)
                        {
                            Ok(old_binding)
                        } else {
                            // Otherwise, fall back to the slow path. Deallocate
                            // the GPU resource bindings, and reallocate them.
                            bind_group_allocator.free(old_binding);
                            let new_binding = bind_group_allocator
                                .allocate_unprepared(&mut self.bind_group_builder, material_layout);
                            *occupied_entry.get_mut() = new_binding;
                            Ok(new_binding)
                        }
                    }
                    Entry::Vacant(vacant_entry) => Ok(*vacant_entry.insert(
                        bind_group_allocator
                            .allocate_unprepared(&mut self.bind_group_builder, material_layout),
                    )),
                }
            }
            Err(AsBindGroupError::RetryNextUpdate) => {
                Err(PrepareAssetError::RetryNextUpdate((*material).clone()))
            }
            Err(AsBindGroupError::CreateBindGroupDirectly) => {
                match material.as_bind_group(
                    material_layout,
                    render_device,
                    pipeline_cache,
                    fallback_buffer,
                    shader_buffer_assets,
                    material_param,
                ) {
                    Ok(prepared_bind_group) => {
                        let bind_group_allocator =
                            bind_group_allocators.get_mut(&TypeId::of::<M>()).unwrap();
                        // Store the resulting bind group directly in the slot.
                        let material_binding_id = bind_group_allocator
                            .allocate_prepared(prepared_bind_group, material_layout.clone());
                        self.insert(material_id.into(), material_binding_id);
                        Ok(material_binding_id)
                    }
                    Err(AsBindGroupError::RetryNextUpdate) => {
                        Err(PrepareAssetError::RetryNextUpdate((*material).clone()))
                    }
                    Err(other) => Err(PrepareAssetError::AsBindGroupError(other)),
                }
            }
            Err(other) => Err(PrepareAssetError::AsBindGroupError(other)),
        };

        // Clear out the bind group builder (but retain its heap allocations; we
        // don't want to allocate here).
        self.bind_group_builder.clear();

        result
    }

    /// Removes a material asset from the render world.
    pub fn unload_material<M>(
        &mut self,
        source_asset: AssetId<M>,
        bind_group_allocators: &mut MaterialBindGroupAllocators,
    ) where
        M: Asset + AsBindGroup + Clone,
    {
        if let Some(material_binding_id) = self.remove(&source_asset.untyped()) {
            let bind_group_allocator = bind_group_allocators.get_mut(&TypeId::of::<M>()).unwrap();
            bind_group_allocator.free(material_binding_id);
        }
    }
}

/// A resource that caches resolved shader handles for a specific material type.
#[derive(Resource)]
pub struct MaterialShaders<M: PointCloudMaterial> {
    pub(crate) shaders: SmallVec<[(InternedShaderLabel, Handle<Shader>); 6]>,
    _marker: PhantomData<M>,
}

impl<M: PointCloudMaterial> Default for MaterialShaders<M> {
    fn default() -> Self {
        Self {
            shaders: SmallVec::new(),
            _marker: PhantomData,
        }
    }
}

impl<M: PointCloudMaterial> MaterialShaders<M> {
    pub fn with_shader_cache(
        shaders: SmallVec<[(InternedShaderLabel, Handle<Shader>); 6]>,
    ) -> Self {
        Self {
            shaders,
            _marker: PhantomData,
        }
    }
}

pub(super) fn initialize_material_shaders<M: PointCloudMaterial>(
    render_world: &World,
) -> SmallVec<[(InternedShaderLabel, Handle<Shader>); 6]> {
    let asset_server = render_world.resource::<AssetServer>();
    let mut shaders = SmallVec::new();

    let mut add_shader = |label: InternedShaderLabel, shader_ref: ShaderRef| {
        let maybe_shader = match shader_ref {
            ShaderRef::Default => None,
            ShaderRef::Handle(handle) => Some(handle),
            ShaderRef::Path(path) => Some(asset_server.load(path)),
        };
        if let Some(shader) = maybe_shader {
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

    // #[cfg(feature = "meshlet")]
    // {
    //     add_shader(
    //         MeshletFragmentShader.intern(),
    //         M::meshlet_mesh_fragment_shader(),
    //     );
    //     add_shader(
    //         MeshletPrepassFragmentShader.intern(),
    //         M::meshlet_mesh_prepass_fragment_shader(),
    //     );
    //     add_shader(
    //         MeshletDeferredFragmentShader.intern(),
    //         M::meshlet_mesh_deferred_fragment_shader(),
    //     );
    // }

    shaders
}
