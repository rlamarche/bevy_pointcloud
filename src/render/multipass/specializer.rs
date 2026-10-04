use bevy::{
    asset::Handle,
    core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT,
    ecs::{error::BevyError, resource::Resource},
    material::{
        descriptor::{CachedRenderPipelineId, FragmentState, RenderPipelineDescriptor},
        specialize::SpecializedMeshPipelineError,
    },
    pbr::MeshPipelineKey,
    platform::{
        collections::{
            hash_map::{Entry, VacantEntry},
            HashMap,
        },
        hash::FixedHasher,
    },
    render::render_resource::{
        binding_types::{texture_2d, texture_2d_multisampled},
        Canonical, ColorTargetState, ColorWrites, CompareFunction, DepthBiasState,
        DepthStencilState, MultisampleState, PipelineCache, RenderPipeline, Specializer,
        SpecializerKey, StencilFaceState, StencilState,
    },
    shader::{Shader, ShaderDefVal},
    utils::default,
};

use std::{hash::Hash, sync::Arc};

use crate::render::{
    multipass::FullscreenMaterialPassPipeline, ErasedViewSettingsKey, PassProperties,
};

#[derive(PartialEq, Eq, Hash, Clone, SpecializerKey)]
pub struct FullscreenPassMaterialPipelineKey {
    pub view_key: MeshPipelineKey,
    pub settings_key: ErasedViewSettingsKey,
    pub pass: usize,
}

/// A trait that allows constructing different variants of a render pipeline from a key and the
/// particular mesh's vertex buffer layout.
///
/// See [`SpecializedMeshPipelines`] for more info.
pub trait SpecializedFullscreenPointCloudPipeline {
    /// The key that defines each "variant" of the render pipeline.
    type Key: Clone + Hash + PartialEq + Eq;

    /// Construct a new render pipeline based on the provided key and vertex layout.
    ///
    /// The returned pipeline descriptor should have a single vertex buffer, which is derived from
    /// `layout`.
    fn specialize(
        &self,
        key: Self::Key,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError>;
}

/// A cache of different variants of a render pipeline based on a key and the particular mesh's
/// vertex buffer layout.
#[derive(Resource)]
pub struct SpecializedFullscreenPointCloudPipelines<S: SpecializedFullscreenPointCloudPipeline> {
    view_layout_cache: HashMap<S::Key, CachedRenderPipelineId>,
}

impl<S: SpecializedFullscreenPointCloudPipeline> Default
    for SpecializedFullscreenPointCloudPipelines<S>
{
    fn default() -> Self {
        Self {
            view_layout_cache: Default::default(),
        }
    }
}

impl<S: SpecializedFullscreenPointCloudPipeline> SpecializedFullscreenPointCloudPipelines<S> {
    /// Construct a new render pipeline based on the provided key and the mesh's vertex buffer
    /// layout.
    #[inline]
    pub fn specialize(
        &mut self,
        cache: &PipelineCache,
        pipeline_specializer: &S,
        key: S::Key,
    ) -> Result<CachedRenderPipelineId, SpecializedMeshPipelineError> {
        return match self.view_layout_cache.entry(key.clone()) {
            Entry::Occupied(entry) => Ok(*entry.into_mut()),
            Entry::Vacant(entry) => specialize_slow(cache, pipeline_specializer, key, entry),
        };

        #[cold]
        fn specialize_slow<S>(
            cache: &PipelineCache,
            specialize_pipeline: &S,
            key: S::Key,
            entry: VacantEntry<S::Key, CachedRenderPipelineId, FixedHasher>,
        ) -> Result<CachedRenderPipelineId, SpecializedMeshPipelineError>
        where
            S: SpecializedFullscreenPointCloudPipeline,
        {
            let descriptor = specialize_pipeline
                .specialize(key.clone())
                .map_err(|mut err| {
                    {
                        let SpecializedMeshPipelineError::MissingVertexAttribute(err) = &mut err;
                        err.pipeline_type = Some(core::any::type_name::<S>());
                    }
                    err
                })?;

            Ok(*entry.insert(cache.queue_render_pipeline(descriptor)))
        }
    }
}

pub struct FullscreenMaterialPipelineSpecializer {
    pub vertex_shader: Option<Handle<Shader>>,
    pub fragment_shader: Option<Handle<Shader>>,
    pub fullscreen_pipeline: FullscreenMaterialPassPipeline,
    pub pass: Arc<PassProperties>,
}

impl Specializer<RenderPipeline> for FullscreenMaterialPipelineSpecializer {
    type Key = FullscreenPassMaterialPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        descriptor: &mut RenderPipelineDescriptor,
    ) -> Result<Canonical<Self::Key>, BevyError> {
        let target_format = match &self.pass.output {
            crate::render::PassOutput::TransientTarget(transient_target) => transient_target.format,
            crate::render::PassOutput::MainColorTarget => key.view_key.target_format(),
        };

        let fragment = descriptor.fragment_mut()?;
        let color_target_state = ColorTargetState {
            format: target_format,
            blend: self.pass.blend,
            write_mask: ColorWrites::ALL,
        };
        fragment.set_target(0, color_target_state);

        Ok(key)
    }
}

impl SpecializedFullscreenPointCloudPipeline for FullscreenMaterialPassPipeline {
    type Key = FullscreenPassMaterialPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut vertex_state = self.vertex_state.clone();
        vertex_state
            .shader_defs
            .push(ShaderDefVal::UInt("POINTCLOUD_BIND_GROUP".into(), 0));

        if key.view_key.msaa_samples() > 1 {
            vertex_state.shader_defs.push("MULTISAMPLED".into());
        };

        let desc = RenderPipelineDescriptor {
            label: Some("fullscreen_material_pass_pipeline".into()),
            layout: vec![self.point_cloud_layout.clone()],
            vertex: vertex_state,
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                // TODO: make it customizable
                depth_write_enabled: Some(true),
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: StencilState {
                    front: StencilFaceState::IGNORE,
                    back: StencilFaceState::IGNORE,
                    read_mask: 0,
                    write_mask: 0,
                },
                bias: DepthBiasState {
                    constant: 0,
                    slope_scale: 0.0,
                    clamp: 0.0,
                },
            }),
            multisample: MultisampleState {
                count: key.view_key.msaa_samples(),
                mask: !0,
                // TODO: make it customizable
                alpha_to_coverage_enabled: false,
            },
            ..default()
        };

        Ok(desc)
    }
}

impl SpecializedFullscreenPointCloudPipeline for FullscreenMaterialPipelineSpecializer {
    type Key = FullscreenPassMaterialPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut descriptor = self.fullscreen_pipeline.specialize(key.clone())?;

        if let Some(pointcloud_layout) = descriptor.layout.last_mut() {
            for input in &self.pass.inputs {
                let entry = if key.view_key.msaa_samples() > 1 {
                    texture_2d_multisampled(input.texture_sample_type)
                        .build(input.binding_slot, input.visibility)
                } else {
                    texture_2d(input.texture_sample_type)
                        .build(input.binding_slot, input.visibility)
                };
                pointcloud_layout.entries.push(entry);
            }
        }

        if let Some(vertex_shader) = &self.vertex_shader {
            descriptor.vertex.shader = vertex_shader.clone();
        }

        let target_format = match &self.pass.output {
            crate::render::PassOutput::TransientTarget(transient_target) => transient_target.format,
            crate::render::PassOutput::MainColorTarget => key.view_key.target_format(),
        };

        let color_target_state = ColorTargetState {
            format: target_format,
            blend: self.pass.blend,
            write_mask: ColorWrites::ALL,
        };

        if let Some(fragment_shader) = &self.fragment_shader {
            let fragment = FragmentState {
                shader: fragment_shader.clone(),
                shader_defs: descriptor.vertex.shader_defs.clone(),
                targets: vec![Some(color_target_state)],
                ..default()
            };

            descriptor.fragment = Some(fragment);
        }

        if let Some(depth_stencil) = descriptor.depth_stencil.as_mut() {
            depth_stencil.depth_write_enabled = self.pass.depth_write_enabled;
        }

        Ok(descriptor)
    }
}
