mod components;
mod phase;

use bevy::{
    app::{App, Plugin},
    asset::{embedded_asset, load_embedded_asset, AssetServer, Handle},
    camera::{Camera, Camera3d},
    core_pipeline::{
        core_3d::{Opaque3dBatchSetKey, Opaque3dBinKey},
        prepass::*,
        Core3d, Core3dSystems,
    },
    ecs::{
        entity::EntityHashMap,
        prelude::*,
        system::{
            lifetimeless::{Read, SRes},
            SystemParam, SystemParamItem, SystemState,
        },
    },
    log::{debug, error, info, warn},
    material::{key::ErasedMeshPipelineKey, OpaqueRendererMethod},
    math::{Mat4, Vec4},
    mesh::MeshVertexBufferLayoutRef,
    pbr::{
        alpha_mode_pipeline_key, collect_meshes_for_gpu_building, set_mesh_motion_vector_flags,
        skins_use_uniform_buffers, MeshLayouts, MeshPipeline, MeshPipelineKey,
        RenderMeshInstanceFlags, RenderMeshInstances, SetMeshViewBindingArrayBindGroup, ShadowView,
    },
    prelude::{Deref, DerefMut},
    render::{
        camera::PendingQueues,
        globals::{GlobalsBuffer, GlobalsUniform},
        mesh::{allocator::MeshAllocator, RenderMesh},
        render_asset::{prepare_assets, RenderAssets},
        render_phase::*,
        render_resource::{
            binding_types::{
                storage_buffer_read_only_sized, storage_buffer_sized, uniform_buffer,
                uniform_buffer_sized,
            },
            *,
        },
        renderer::{RenderAdapter, RenderDevice, RenderQueue},
        sync_world::RenderEntity,
        view::{
            ExtractedView, Msaa, RenderVisibilityRanges, RenderVisibleEntities, RetainedViewEntity,
            ViewUniform, ViewUniformOffset, ViewUniforms, VISIBILITY_RANGES_STORAGE_BUFFER_COUNT,
        },
        Extract, ExtractSchedule, GpuResourceAppExt, Render, RenderApp, RenderDebugFlags,
        RenderStartup, RenderSystems,
    },
    shader::{load_shader_library, Shader, ShaderDefVal},
};
use core::any::TypeId;

use bevy::{
    ecs::{change_detection::Tick, system::SystemChangeTick},
    platform::{
        collections::{HashMap, HashSet},
        hash::FixedHasher,
    },
    render::{erased_render_asset::ErasedRenderAssets, sync_world::MainEntity},
};
use std::{num::NonZero, sync::Arc};

use crate::{
    init_material_pipeline, init_point_cloud_pipeline, BinnedRenderPhaseExt,
    DrawPointCloudInstanced, ErasedMaterialPipelineKey, ErasedSplatPipelineKey, MaterialPipeline,
    MaterialProperties, MultiPassOpaqueDrawFunction, PointCloudChunk3d,
    PointCloudDirtySpecializations, PointCloudPipeline, PointCloudTopologyKind, PreparedMaterial,
    PrepassOpaqueDepthOnlyDrawFunction, RenderPointCloudChunkInstances, RenderPointCloudInstances,
    RenderPointCloudMaterialInstances, SetMaterialBindGroup, SetPointCloudBindGroup,
    SpecializedPointCloudPipeline, SpecializedPointCloudPipelines, SplatPipelineKey,
};

pub use components::*;
pub use phase::*;

/// Sets up everything required to use the prepass pipeline.
///
/// This does not add the actual prepasses, see [`PrepassPlugin`] for that.
pub struct MultipassPipelinePlugin;

impl Plugin for MultipassPipelinePlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "multipass.wgsl");

        load_shader_library!(app, "multipass_bindings.wgsl");
        load_shader_library!(app, "multipass_utils.wgsl");
        load_shader_library!(app, "multipass_io.wgsl");

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_systems(
                RenderStartup,
                (
                    init_multipass_pipeline
                        .after(init_material_pipeline)
                        .after(init_point_cloud_pipeline),
                    init_multipass_view_bind_group,
                )
                    .chain(),
            )
            .add_systems(
                Render,
                prepare_multipass_view_bind_group.in_set(RenderSystems::PrepareBindGroups),
            )
            .init_gpu_resource::<SpecializedPointCloudPipelines<MultipassPipelineSpecializer>>();
    }
}

/// Sets up the prepasses for a material.
///
/// This depends on the [`PrepassPipelinePlugin`].
pub struct MultipassPlugin {
    /// Debugging flags that can optionally be set when constructing the renderer.
    pub debug_flags: RenderDebugFlags,
}

impl MultipassPlugin {
    /// Creates a new [`PrepassPlugin`] with the given debug flags.
    pub fn new(debug_flags: RenderDebugFlags) -> Self {
        MultipassPlugin { debug_flags }
    }
}

impl Plugin for MultipassPlugin {
    fn build(&self, app: &mut App) {
        let no_prepass_plugin_loaded = app
            .world()
            .get_resource::<AnyMultipassPluginLoaded>()
            .is_none();

        if no_prepass_plugin_loaded {
            app.insert_resource(AnyMultipassPluginLoaded);
        }

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<DrawFunctions<Opaque3dMultipass>>()
            .init_resource::<ViewBinnedRenderPhases<Opaque3dMultipass>>();

        render_app
            .init_gpu_resource::<ViewKeyMultipassCache>()
            .init_gpu_resource::<SpecializedMultipassMaterialPipelineCache>()
            .init_gpu_resource::<PendingMultipassMeshMaterialQueues>()
            .add_render_command::<Opaque3dMultipass, DrawMultipass>()
            .add_systems(ExtractSchedule, extract_camera_multipass_phase)
            .add_systems(
                Render,
                (
                    specialize_multipass_material_meshes
                        .in_set(RenderSystems::PrepareMeshes)
                        .after(prepare_assets::<RenderMesh>)
                        .after(collect_meshes_for_gpu_building)
                        .after(set_mesh_motion_vector_flags),
                    queue_multipass_material_meshes.in_set(RenderSystems::QueueMeshes),
                ),
            );

        // #[cfg(feature = "meshlet")]
        // render_app.add_systems(
        //     Render,
        //     prepare_material_meshlet_meshes_prepass
        //         .in_set(RenderSystems::QueueMeshes)
        //         .before(queue_material_meshlet_meshes)
        //         .run_if(resource_exists::<InstanceManager>),
        // );
    }
}

#[derive(Resource)]
struct AnyMultipassPluginLoaded;

#[derive(Resource, Clone)]
pub struct MultipassPipeline {
    pub view_layout_motion_vectors: BindGroupLayoutDescriptor,
    pub view_layout_no_motion_vectors: BindGroupLayoutDescriptor,
    pub mesh_layouts: MeshLayouts,
    pub default_prepass_shader: Handle<Shader>,

    /// Whether skins will use uniform buffers on account of storage buffers
    /// being unavailable on this platform.
    pub skins_use_uniform_buffers: bool,
    pub depth_clip_control_supported: bool,

    pub material_pipeline: MaterialPipeline,
    pub point_cloud_pipeline: PointCloudPipeline,
}

pub(crate) fn buffer_layout(
    buffer_binding_type: BufferBindingType,
    has_dynamic_offset: bool,
    min_binding_size: Option<NonZero<u64>>,
) -> BindGroupLayoutEntryBuilder {
    match buffer_binding_type {
        BufferBindingType::Uniform => uniform_buffer_sized(has_dynamic_offset, min_binding_size),
        BufferBindingType::Storage { read_only } => {
            if read_only {
                storage_buffer_read_only_sized(has_dynamic_offset, min_binding_size)
            } else {
                storage_buffer_sized(has_dynamic_offset, min_binding_size)
            }
        }
    }
}

/// How many texture bindings are used in the fragment shader, *not* counting
/// environment maps or irradiance volumes.
/// Keep in sync with Bevy
const STANDARD_MATERIAL_FRAGMENT_SHADER_MIN_TEXTURE_BINDINGS: usize = 16;

pub fn init_multipass_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_adapter: Res<RenderAdapter>,
    mesh_pipeline: Res<MeshPipeline>,
    material_pipeline: Res<MaterialPipeline>,
    point_cloud_pipeline: Res<PointCloudPipeline>,
    asset_server: Res<AssetServer>,
) {
    let visibility_ranges_buffer_binding_type =
        render_device.get_supported_read_only_binding_type(VISIBILITY_RANGES_STORAGE_BUFFER_COUNT);

    let view_layout_motion_vectors = BindGroupLayoutDescriptor::new(
        "prepass_view_layout_motion_vectors",
        &BindGroupLayoutEntries::with_indices(
            ShaderStages::VERTEX_FRAGMENT,
            (
                // View
                (0, uniform_buffer::<ViewUniform>(true)),
                // Globals
                (1, uniform_buffer::<GlobalsUniform>(false)),
                // PreviousViewUniforms
                (2, uniform_buffer::<PreviousViewData>(true)),
                // VisibilityRanges
                (
                    14,
                    buffer_layout(
                        visibility_ranges_buffer_binding_type,
                        false,
                        Some(Vec4::min_size()),
                    )
                    .visibility(ShaderStages::VERTEX),
                ),
            ),
        ),
    );

    let view_layout_no_motion_vectors = BindGroupLayoutDescriptor::new(
        "prepass_view_layout_no_motion_vectors",
        &BindGroupLayoutEntries::with_indices(
            ShaderStages::VERTEX_FRAGMENT,
            (
                // View
                (0, uniform_buffer::<ViewUniform>(true)),
                // Globals
                (1, uniform_buffer::<GlobalsUniform>(false)),
                // VisibilityRanges
                (
                    14,
                    buffer_layout(
                        visibility_ranges_buffer_binding_type,
                        false,
                        Some(Vec4::min_size()),
                    )
                    .visibility(ShaderStages::VERTEX),
                ),
            ),
        ),
    );

    let depth_clip_control_supported = render_device
        .features()
        .contains(WgpuFeatures::DEPTH_CLIP_CONTROL);
    commands.insert_resource(MultipassPipeline {
        view_layout_motion_vectors,
        view_layout_no_motion_vectors,
        mesh_layouts: mesh_pipeline.mesh_layouts.clone(),
        // default_prepass_shader: asset_server.load("shaders/prepass_dev.wgsl"),
        default_prepass_shader: load_embedded_asset!(asset_server.as_ref(), "multipass.wgsl"),
        skins_use_uniform_buffers: skins_use_uniform_buffers(&render_device.limits()),
        depth_clip_control_supported,
        material_pipeline: material_pipeline.clone(),
        point_cloud_pipeline: point_cloud_pipeline.clone(),
    });
}

pub struct MultipassPipelineSpecializer {
    pub pipeline: MultipassPipeline,
    pub properties: Arc<MaterialProperties>,
}

impl SpecializedPointCloudPipeline for MultipassPipelineSpecializer {
    type Key = ErasedMaterialPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        splat_layout: &MeshVertexBufferLayoutRef,
        instance_layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut shader_defs = Vec::new();

        if self.properties.bindless {
            shader_defs.push("BINDLESS".into());
        }

        let concrete_mesh_key: MeshPipelineKey = key.mesh_key.downcast();
        let concrete_splat_key: SplatPipelineKey = key.splat_key.downcast();
        let mut descriptor = self.pipeline.specialize(
            (concrete_mesh_key, concrete_splat_key),
            shader_defs,
            splat_layout,
            instance_layout,
            &self.properties,
        )?;

        // This is a bit risky because it's possible to change something that would
        // break the prepass but be fine in the main pass.
        // Since this api is pretty low-level it doesn't matter that much, but it is a potential
        // issue.
        if let Some(specialize) = self.properties.user_specialize {
            specialize(
                &self.pipeline.material_pipeline,
                &mut descriptor,
                splat_layout,
                instance_layout,
                key,
            )?;
        }

        Ok(descriptor)
    }
}

impl MultipassPipeline {
    fn specialize(
        &self,
        (mesh_key, splat_key): (MeshPipelineKey, SplatPipelineKey),
        shader_defs: Vec<ShaderDefVal>,
        splat_layout: &MeshVertexBufferLayoutRef,
        instance_layout: &MeshVertexBufferLayoutRef,
        material_properties: &MaterialProperties,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let descriptor = self.point_cloud_pipeline.specialize(
            (mesh_key, splat_key),
            splat_layout,
            instance_layout,
        )?;

        Ok(descriptor)
    }
}

// Extract the render phases for the prepass
pub fn extract_camera_previous_view_data(
    mut commands: Commands,
    cameras_3d: Extract<Query<(RenderEntity, &Camera, Option<&PreviousViewData>), With<Camera3d>>>,
) {
    for (entity, camera, maybe_previous_view_data) in cameras_3d.iter() {
        let mut entity = commands
            .get_entity(entity)
            .expect("Camera entity wasn't synced.");
        if camera.is_active {
            if let Some(previous_view_data) = maybe_previous_view_data {
                entity.insert(previous_view_data.clone());
            }
        } else {
            entity.remove::<PreviousViewData>();
        }
    }
}

pub fn prepare_previous_view_uniforms(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut previous_view_uniforms: ResMut<PreviousViewUniforms>,
    views: Query<
        (Entity, &ExtractedView, Option<&PreviousViewData>),
        Or<(With<Camera3d>, With<ShadowView>)>,
    >,
) {
    let views_iter = views.iter();
    let view_count = views_iter.len();
    let Some(mut writer) =
        previous_view_uniforms
            .uniforms
            .get_writer(view_count, &render_device, &render_queue)
    else {
        return;
    };

    for (entity, camera, maybe_previous_view_uniforms) in views_iter {
        let prev_view_data = match maybe_previous_view_uniforms {
            Some(previous_view) => previous_view.clone(),
            None => {
                let world_from_view = camera.world_from_view.affine();
                let view_from_world = Mat4::from(world_from_view.inverse());
                let view_from_clip = camera.clip_from_view.inverse();

                PreviousViewData {
                    view_from_world,
                    clip_from_world: camera.clip_from_view * view_from_world,
                    clip_from_view: camera.clip_from_view,
                    world_from_clip: Mat4::from(world_from_view) * view_from_clip,
                    view_from_clip,
                }
            }
        };

        commands.entity(entity).insert(PreviousViewUniformOffset {
            offset: writer.write(&prev_view_data),
        });
    }
}

#[derive(Resource)]
pub struct MultipassViewBindGroup {
    pub motion_vectors: Option<BindGroup>,
    pub no_motion_vectors: Option<BindGroup>,
}

pub fn init_multipass_view_bind_group(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<MultipassPipeline>,
) {
    commands.insert_resource(MultipassViewBindGroup {
        motion_vectors: None,
        no_motion_vectors: None,
    });
}

pub fn prepare_multipass_view_bind_group(
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    prepass_pipeline: Res<MultipassPipeline>,
    view_uniforms: Res<ViewUniforms>,
    globals_buffer: Res<GlobalsBuffer>,
    previous_view_uniforms: Res<PreviousViewUniforms>,
    visibility_ranges: Res<RenderVisibilityRanges>,
    mut prepass_view_bind_group: ResMut<MultipassViewBindGroup>,
) {
    if let (Some(view_binding), Some(globals_binding), Some(visibility_ranges_buffer)) = (
        view_uniforms.uniforms.binding(),
        globals_buffer.buffer.binding(),
        visibility_ranges.buffer().buffer(),
    ) {
        prepass_view_bind_group.no_motion_vectors = Some(render_device.create_bind_group(
            "prepass_view_no_motion_vectors_bind_group",
            &pipeline_cache.get_bind_group_layout(&prepass_pipeline.view_layout_no_motion_vectors),
            &BindGroupEntries::with_indices((
                (0, view_binding.clone()),
                (1, globals_binding.clone()),
                (14, visibility_ranges_buffer.as_entire_binding()),
            )),
        ));

        if let Some(previous_view_uniforms_binding) = previous_view_uniforms.uniforms.binding() {
            prepass_view_bind_group.motion_vectors = Some(render_device.create_bind_group(
                "prepass_view_motion_vectors_bind_group",
                &pipeline_cache.get_bind_group_layout(&prepass_pipeline.view_layout_motion_vectors),
                &BindGroupEntries::with_indices((
                    (0, view_binding),
                    (1, globals_binding),
                    (2, previous_view_uniforms_binding),
                    (14, visibility_ranges_buffer.as_entire_binding()),
                )),
            ));
        }
    }
}

/// Stores the [`SpecializedPrepassMaterialViewPipelineCache`] for each view.
#[derive(Resource, Deref, DerefMut, Default)]
pub struct SpecializedMultipassMaterialPipelineCache {
    // view_entity -> view pipeline cache
    #[deref]
    map: HashMap<RetainedViewEntity, SpecializedMultipassMaterialViewPipelineCache>,
}

/// Stores the cached render pipeline ID for each entity in a single view, as
/// well as the last time it was changed.
#[derive(Deref, DerefMut, Default)]
pub struct SpecializedMultipassMaterialViewPipelineCache {
    // material entity -> (tick, pipeline_id, draw_function)
    #[deref]
    map: EntityHashMap<(Tick, CachedRenderPipelineId, DrawFunctionId)>,
}

#[derive(Resource, Deref, DerefMut, Default, Clone)]
pub struct ViewKeyMultipassCache(HashMap<RetainedViewEntity, MeshPipelineKey>);

pub(crate) struct MultipassSpecializationWorkItem {
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

impl core::fmt::Debug for MultipassSpecializationWorkItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultipassSpecializationWorkItem")
            .field("render_entity", &self.render_entity)
            .field("visible_entity", &self.visible_entity)
            .field("retained_view_entity", &self.retained_view_entity)
            .field("mesh_key", &self.mesh_key)
            .field("splat_key", &self.splat_key)
            .field("splat_layout", &self.splat_layout)
            .field("instance_layout", &self.instance_layout)
            // .field("properties", &self.properties)
            .field("material_type_id", &self.material_type_id)
            .finish()
    }
}

/// Holds all entities with mesh materials for which the prepass couldn't be
/// specialized and/or queued because their materials hadn't loaded yet.
///
/// See the [`PendingQueues`] documentation for more information.
#[derive(Default, Deref, DerefMut, Resource)]
pub struct PendingMultipassMeshMaterialQueues(pub PendingQueues);

#[derive(SystemParam)]
pub(crate) struct SpecializeMultipassSystemParam<'w, 's> {
    render_meshes: Res<'w, RenderAssets<RenderMesh>>,
    render_materials: Res<'w, ErasedRenderAssets<PreparedMaterial>>,
    render_mesh_instances: Res<'w, RenderMeshInstances>,
    render_material_instances: Res<'w, RenderPointCloudMaterialInstances>,
    render_point_cloud_instances: Res<'w, RenderPointCloudInstances>,
    render_point_cloud_chunk_instances: Res<'w, RenderPointCloudChunkInstances>,
    render_visibility_ranges: Res<'w, RenderVisibilityRanges>,
    view_key_cache: Res<'w, ViewKeyMultipassCache>,
    views: Query<'w, 's, (&'static ExtractedView, &'static RenderVisibleEntities)>,
    opaque_multipass_render_phases: Res<'w, ViewBinnedRenderPhases<Opaque3dMultipass>>,
    specialized_multipass_material_pipeline_cache:
        ResMut<'w, SpecializedMultipassMaterialPipelineCache>,
    pending_multipass_mesh_material_queues: ResMut<'w, PendingMultipassMeshMaterialQueues>,
    dirty_specializations: Res<'w, PointCloudDirtySpecializations>,
    this_run: SystemChangeTick,
}

pub(crate) fn specialize_multipass_material_meshes(
    world: &mut World,
    state: &mut SystemState<SpecializeMultipassSystemParam>,
    mut work_items: Local<Vec<MultipassSpecializationWorkItem>>,
    mut removals: Local<Vec<(RetainedViewEntity, Entity)>>,
    mut all_views: Local<HashSet<RetainedViewEntity, FixedHasher>>,
) {
    work_items.clear();
    removals.clear();
    all_views.clear();

    let this_run;

    {
        let SpecializeMultipassSystemParam {
            render_meshes,
            render_materials,
            render_mesh_instances,
            render_material_instances,
            render_point_cloud_instances,
            render_point_cloud_chunk_instances,
            render_visibility_ranges,
            view_key_cache,
            views,
            opaque_multipass_render_phases,
            mut specialized_multipass_material_pipeline_cache,
            mut pending_multipass_mesh_material_queues,
            dirty_specializations,
            this_run: system_change_tick,
        } = state.get_mut(world).unwrap();

        this_run = system_change_tick.this_run();

        for (view, visible_entities) in &views {
            if !opaque_multipass_render_phases.contains_key(&view.retained_view_entity) {
                continue;
            }

            let Some(view_key) = view_key_cache.get(&view.retained_view_entity) else {
                continue;
            };

            all_views.insert(view.retained_view_entity);

            let Some(visible_entities_class) = visible_entities.get::<PointCloudChunk3d>() else {
                continue;
            };

            // Fetch the pending mesh material queues for this view.
            let view_pending_multipass_mesh_material_queues =
                pending_multipass_mesh_material_queues
                    .prepare_for_new_frame(view.retained_view_entity);

            // Initialize the pending queues.
            let mut maybe_specialized_multipass_material_pipeline_cache =
                specialized_multipass_material_pipeline_cache.get_mut(&view.retained_view_entity);

            // Remove cached pipeline IDs corresponding to entities that
            // either have been removed or need to be respecialized.
            if let Some(ref mut specialized_multipass_material_pipeline_cache) =
                maybe_specialized_multipass_material_pipeline_cache
            {
                if dirty_specializations
                    .must_wipe_specializations_for_view(view.retained_view_entity)
                {
                    specialized_multipass_material_pipeline_cache.clear();
                } else {
                    for (&renderable_entity, &_main_entity) in
                        dirty_specializations.iter_to_despecialize()
                    {
                        specialized_multipass_material_pipeline_cache.remove(&renderable_entity);
                    }
                }
            }

            // Now process all meshes that need to be specialized.
            for (render_entity, visible_entity) in dirty_specializations.iter_to_specialize(
                view.retained_view_entity,
                visible_entities_class,
                &view_pending_multipass_mesh_material_queues.prev_frame,
            ) {
                if maybe_specialized_multipass_material_pipeline_cache
                    .as_ref()
                    .is_some_and(|specialized_multipass_material_pipeline_cache| {
                        specialized_multipass_material_pipeline_cache.contains_key(render_entity)
                    })
                {
                    continue;
                }

                // our entity is a chunk, we need parent point cloud to get its specialized pipeline
                // & material
                let Some(render_point_cloud_chunk_instance) =
                    render_point_cloud_chunk_instances.get(render_entity)
                else {
                    warn!(
                        "RenderPointCloudChunkInstance not found for entity {:?} in multipass",
                        visible_entity
                    );
                    continue;
                };

                let Some(render_point_cloud_instance) = render_point_cloud_instances
                    .get(&render_point_cloud_chunk_instance.root_entity)
                else {
                    warn!(
                        "RenderPointCloudInstance not found for entity {:?} in multipass",
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
                        "material_instance not found for entity {:?} in multipass",
                        visible_entity
                    );

                    view_pending_multipass_mesh_material_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };

                // get the mesh instance from the root entity
                let Some(mesh_instance) = render_mesh_instances
                    .render_mesh_queue_data(render_point_cloud_chunk_instance.root_entity)
                else {
                    warn!(
                        "mesh_instance not found for entity {:?} in multipass",
                        render_point_cloud_chunk_instance.root_entity
                    );
                    view_pending_multipass_mesh_material_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };

                // get the mesh from the chunk
                let Some(mesh) = render_meshes.get(render_point_cloud_chunk_instance.mesh_asset_id)
                else {
                    warn!(
                        "render_meshes not found for asset id {:?} in multipass",
                        render_point_cloud_chunk_instance.mesh_asset_id
                    );
                    view_pending_multipass_mesh_material_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };

                let Some(material) = render_materials.get(material_instance.asset_id) else {
                    warn!(
                        "render_material not found for entity {:?} in multipass",
                        visible_entity
                    );

                    view_pending_multipass_mesh_material_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };

                let Some(splat_mesh) = render_meshes.get(render_point_cloud_instance.splat) else {
                    debug!("shape mesh not found when specialising multipass");
                    view_pending_multipass_mesh_material_queues
                        .current_frame
                        .insert((*render_entity, *visible_entity));
                    continue;
                };

                if !material.properties.multipass_enabled {
                    // TODO: remove this warn
                    warn!(
                        "multipass not enabled for entity {:?} in multipass",
                        visible_entity
                    );

                    // If the material was previously specialized for multipass, remove it
                    removals.push((view.retained_view_entity, *render_entity));
                    continue;
                }

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

                let mut splat_key = (&render_point_cloud_instance.splat_settings).into();

                if matches!(
                    render_point_cloud_instance.topology,
                    PointCloudTopologyKind::Octree
                ) {
                    splat_key |= SplatPipelineKey::IS_OCTREE;
                }

                work_items.push(MultipassSpecializationWorkItem {
                    render_entity: *render_entity,
                    visible_entity: *visible_entity,
                    retained_view_entity: view.retained_view_entity,
                    mesh_key,
                    splat_key,
                    splat_layout: splat_mesh.layout.clone(),
                    instance_layout: mesh.layout.clone(),
                    properties: material.properties.clone(),
                    material_type_id: material_instance.asset_id.type_id(),
                });
            }
        }

        pending_multipass_mesh_material_queues.expire_stale_views(&all_views);
    }

    for item in work_items.drain(..) {
        info!("Processing work item {:?} for multipass", item);
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

        let draw_function = item
            .properties
            .get_draw_function(MultiPassOpaqueDrawFunction);

        let Some(draw_function) = draw_function else {
            warn!("no draw function");
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
                    .resource_mut::<SpecializedMultipassMaterialPipelineCache>()
                    .entry(item.retained_view_entity)
                    .or_default()
                    .insert(item.render_entity, (this_run, pipeline_id, draw_function));
            }
            Err(err) => error!("{}", err),
        }
    }

    if !removals.is_empty() {
        let mut cache = world.resource_mut::<SpecializedMultipassMaterialPipelineCache>();
        for (view, entity) in removals.drain(..) {
            if let Some(view_cache) = cache.get_mut(&view) {
                view_cache.remove(&entity);
            }
        }
    }

    world
        .resource_mut::<SpecializedMultipassMaterialPipelineCache>()
        .retain(|view, _| all_views.contains(view));
}

pub fn queue_multipass_material_meshes(
    render_materials: Res<ErasedRenderAssets<PreparedMaterial>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    render_material_instances: Res<RenderPointCloudMaterialInstances>,
    render_point_cloud_chunk_instances: Res<RenderPointCloudChunkInstances>,
    mesh_allocator: Res<MeshAllocator>,
    mut opaque_multipass_render_phases: ResMut<ViewBinnedRenderPhases<Opaque3dMultipass>>,
    views: Query<(&ExtractedView, &RenderVisibleEntities)>,
    specialized_material_pipeline_cache: Res<SpecializedMultipassMaterialPipelineCache>,
    mut pending_multipass_mesh_material_queues: ResMut<PendingMultipassMeshMaterialQueues>,
    dirty_specializations: Res<PointCloudDirtySpecializations>,
) {
    for (view, visible_entities) in &views {
        let mut opaque_phase = opaque_multipass_render_phases.get_mut(&view.retained_view_entity);

        let Some(view_specialized_material_pipeline_cache) =
            specialized_material_pipeline_cache.get(&view.retained_view_entity)
        else {
            warn!("no specialized_material_pipeline_cache for multipass");
            continue;
        };

        // Skip if there's no place to put the mesh.
        if opaque_phase.is_none() {
            warn!("no phase for multipass");
            continue;
        }

        let Some(visible_entities_class) = visible_entities.get::<PointCloudChunk3d>() else {
            warn!("no visible entities for multipass");
            continue;
        };

        // First, remove meshes that need to be respecialized, and those that were removed, from the
        // bins.
        for (render_entity, main_entity) in
            dirty_specializations.iter_to_dequeue(view.retained_view_entity, visible_entities_class)
        {
            let Some(render_point_cloud_chunk_instance) = (match render_point_cloud_chunk_instances
                .previous
                .get(render_entity)
            {
                Some(value) => Some(value),
                None => render_point_cloud_chunk_instances.get(render_entity),
            }) else {
                warn!(
                    "RenderPointCloudChunkInstance not found for entity {:?} when removing multipass phase",
                    main_entity
                );
                continue;
            };

            if let Some(ref mut opaque_phase) = opaque_phase {
                opaque_phase.remove_unbatchable_entity_pair(
                    render_entity,
                    &render_point_cloud_chunk_instance.root_entity,
                );
            }
        }

        // Fetch the pending mesh material queues for this view.
        let view_pending_prepass_mesh_material_queues = pending_multipass_mesh_material_queues
            .get_mut(&view.retained_view_entity)
            .expect(
                "View pending prepass mesh material queues should have been created in \
                 `specialize_prepass_material_meshes`",
            );

        // Now iterate through all newly-visible entities and those needing respecialization.
        for (render_entity, visible_entity) in dirty_specializations.iter_to_queue(
            view.retained_view_entity,
            visible_entities_class,
            &view_pending_prepass_mesh_material_queues.prev_frame,
        ) {
            let Some(&(_, pipeline_id, draw_function)) =
                view_specialized_material_pipeline_cache.get(render_entity)
            else {
                continue;
            };

            // our entity is a chunk, we need parent point cloud to get its specialized pipeline
            // & material
            let Some(render_point_cloud_chunk_instance) =
                render_point_cloud_chunk_instances.get(render_entity)
            else {
                warn!(
                    "RenderPointCloudChunkInstance not found for entity {:?} for multipass",
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
                warn!("material instance not ready, queue for multipass");
                view_pending_prepass_mesh_material_queues
                    .current_frame
                    .insert((*render_entity, *visible_entity));
                continue;
            };
            // get the mesh instance from the root entity
            let Some(mesh_instance) = render_mesh_instances
                .render_mesh_queue_data(render_point_cloud_chunk_instance.root_entity)
            else {
                warn!("mesh instance not ready, queue for multipass");
                view_pending_prepass_mesh_material_queues
                    .current_frame
                    .insert((*render_entity, *visible_entity));
                continue;
            };
            let Some(material) = render_materials.get(material_instance.asset_id) else {
                warn!("material not ready, queue for prepass");
                view_pending_prepass_mesh_material_queues
                    .current_frame
                    .insert((*render_entity, *visible_entity));
                continue;
            };

            // Fetch the slabs that this mesh resides in.
            let Some(mesh_slabs) =
                mesh_allocator.mesh_slabs(&render_point_cloud_chunk_instance.mesh_asset_id)
            else {
                warn!(
                    "mesh slab not found for visible entity {:?} for prepass",
                    visible_entity
                );
                view_pending_prepass_mesh_material_queues
                    .current_frame
                    .insert((*render_entity, *visible_entity));
                continue;
            };

            if let Some(opaque_phase) = opaque_phase.as_mut() {
                if material.properties.render_method == OpaqueRendererMethod::Deferred {
                    // Even though we aren't going to insert the entity into
                    // a bin, we still want to update its cache entry. That
                    // way, we know we don't need to re-examine it in future
                    // frames.
                    opaque_phase.update_cache(*visible_entity, None);
                    continue;
                }
                let Some(draw_function) = material
                    .properties
                    .get_draw_function(MultiPassOpaqueDrawFunction)
                else {
                    warn!("draw function not found for multipass");
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

                info!(
                    "Add phase for entity {:?}/{:?} ({:?})",
                    render_point_cloud_chunk_instance.root_entity, render_entity, visible_entity
                );
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
        }
    }
}

pub struct SetMultipassViewBindGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetMultipassViewBindGroup<I> {
    type Param = SRes<MultipassViewBindGroup>;
    type ViewQuery = (
        Read<ViewUniformOffset>,
        Has<MotionVectorPrepass>,
        Option<Read<PreviousViewUniformOffset>>,
    );
    type ItemQuery = ();

    #[inline]
    fn render<'w>(
        _item: &P,
        (view_uniform_offset, has_motion_vector_prepass, previous_view_uniform_offset): (
            &'_ ViewUniformOffset,
            bool,
            Option<&'_ PreviousViewUniformOffset>,
        ),
        _entity: Option<()>,
        prepass_view_bind_group: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let prepass_view_bind_group = prepass_view_bind_group.into_inner();

        match previous_view_uniform_offset {
            Some(previous_view_uniform_offset) if has_motion_vector_prepass => {
                pass.set_bind_group(
                    I,
                    prepass_view_bind_group.motion_vectors.as_ref().unwrap(),
                    &[
                        view_uniform_offset.offset,
                        previous_view_uniform_offset.offset,
                    ],
                );
            }
            _ => {
                pass.set_bind_group(
                    I,
                    prepass_view_bind_group.no_motion_vectors.as_ref().unwrap(),
                    &[view_uniform_offset.offset],
                );
            }
        }
        RenderCommandResult::Success
    }
}

pub type DrawMultipass = (
    SetItemPipeline,
    SetMultipassViewBindGroup<0>,
    SetMeshViewBindingArrayBindGroup<1>,
    SetPointCloudBindGroup<2>,
    SetMaterialBindGroup<3>,
    DrawPointCloudInstanced,
);
