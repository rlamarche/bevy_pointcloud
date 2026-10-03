use std::{any::Any, sync::Arc};

use bevy::{
    ecs::world::World,
    log::info,
    material::{
        descriptor::{CachedRenderPipelineId, RenderPipelineDescriptor},
        specialize::SpecializedMeshPipelineError,
    },
    mesh::MeshVertexBufferLayoutRef,
    pbr::MeshPipelineKey,
    render::render_resource::binding_types::{texture_2d, texture_2d_multisampled},
    shader::ShaderDefVal,
};

use crate::render::{
    ErasedPointCloudMaterialPipelineKey, MaterialFragmentShader, MaterialVertexShader, PassOutput,
    PointCloudMaterialPipeline, PointCloudMaterialProperties, SpecializedPointCloudPipeline,
    SplatPipelineKey,
};

pub struct PointCloudMaterialPipelineSpecializer {
    pub(crate) pipeline: PointCloudMaterialPipeline,
    pub(crate) properties: Arc<PointCloudMaterialProperties>,
}

impl SpecializedPointCloudPipeline for PointCloudMaterialPipelineSpecializer {
    type Key = ErasedPointCloudMaterialPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        splat_layout: &MeshVertexBufferLayoutRef,
        instance_layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        info!("Specialize pointcloud pipeline for pass {:?}", key.pass);

        let concrete_mesh_key: MeshPipelineKey = key.mesh_key.downcast();
        let concrete_splat_key: SplatPipelineKey = key.splat_key.downcast();
        let mut descriptor = self.pipeline.pointcloud_pipeline.specialize(
            (
                concrete_mesh_key,
                concrete_splat_key,
                key.view_settings_key.clone(),
                key.pass,
            ),
            splat_layout,
            instance_layout,
        )?;

        let mut shader_defs: Vec<ShaderDefVal> = Vec::new();

        // set the view settings uniform layout
        if let Some(view_settings_layout_entry) =
            self.properties.view_settings_layout_entry.as_ref()
        {
            descriptor
                .layout
                .last_mut()
                .unwrap()
                .entries
                .push(*view_settings_layout_entry);
            shader_defs.push("POINTCLOUD_VIEW_SETTINGS".into());
        }

        let material_bind_group_index = descriptor.layout.len();

        shader_defs.push(ShaderDefVal::UInt(
            "MATERIAL_BIND_GROUP".into(),
            material_bind_group_index as u32,
        ));

        // specialize the pass
        if let Some(pass) = key.pass {
            let pass_properties = &self.properties.passes[pass];

            // add useful shaderdefs (eg: `POINTCLOUD_PASS_DEPTH` and `POINTCLOUD_PASS_0`)
            shader_defs.push(ShaderDefVal::UInt("POINTCLOUD_PASS".into(), pass as u32));

            shader_defs.push(format!("POINTCLOUD_PASS_{}", pass).into());
            shader_defs
                .push(format!("POINTCLOUD_PASS_{}", pass_properties.name.to_uppercase()).into());

            if let Some(vertex_shader) = pass_properties.get_shader(MaterialVertexShader) {
                descriptor.vertex.shader = vertex_shader.clone();
            }

            if let Some(fragment) = descriptor.fragment.as_mut() {
                if let Some(fragment_shader) = pass_properties.get_shader(MaterialFragmentShader) {
                    fragment.shader = fragment_shader.clone();
                }
                if let Some(Some(target)) = fragment.targets.get_mut(0) {
                    target.blend = pass_properties.blend;
                }
            }

            if let Some(pointcloud_layout) = descriptor.layout.last_mut() {
                for input in &pass_properties.inputs {
                    let entry = if concrete_mesh_key.msaa_samples() > 1 {
                        texture_2d_multisampled(input.texture_sample_type)
                            .build(input.binding_slot, input.visibility)
                    } else {
                        texture_2d(input.texture_sample_type)
                            .build(input.binding_slot, input.visibility)
                    };
                    pointcloud_layout.entries.push(entry);
                }
            }

            if let PassOutput::TransientTarget(transient_target) = &pass_properties.output {
                let color_target_state = descriptor.fragment.as_mut().unwrap().targets[0]
                    .as_mut()
                    .unwrap();
                color_target_state.format = transient_target.format;
            }

            // set depth write configuration
            if let Some(depth_stencil) = descriptor.depth_stencil.as_mut() {
                depth_stencil.depth_write_enabled = pass_properties.depth_write_enabled;
            }
        } else {
            if let Some(vertex_shader) = self.properties.get_shader(MaterialVertexShader) {
                descriptor.vertex.shader = vertex_shader.clone();
            }

            if let Some(fragment_shader) = self.properties.get_shader(MaterialFragmentShader) {
                descriptor.fragment.as_mut().unwrap().shader = fragment_shader.clone();
            }
        }

        descriptor
            .layout
            .push(self.properties.material_layout.as_ref().unwrap().clone());

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

        descriptor.vertex.shader_defs.extend(shader_defs.clone());
        if let Some(ref mut fragment) = descriptor.fragment {
            fragment.shader_defs.extend(shader_defs);
        }

        Ok(descriptor)
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
    ErasedPointCloudMaterialPipelineKey,
    &MeshVertexBufferLayoutRef,
    &MeshVertexBufferLayoutRef,
    &Arc<PointCloudMaterialProperties>,
) -> Result<CachedRenderPipelineId, SpecializedMeshPipelineError>;

/// A type erased function pointer for specializing a material prepass pipeline. The implementation
/// is expected to:
/// - Look up the appropriate specializer from the world
/// - Downcast the erased key to the concrete key type
/// - Call `SpecializedMeshPipelines::specialize` with the specializer and return the resulting
///   pipeline id
pub type PrepassSpecializeFn = fn(
    &mut World,
    ErasedPointCloudMaterialPipelineKey,
    &MeshVertexBufferLayoutRef,
    &MeshVertexBufferLayoutRef,
    &Arc<PointCloudMaterialProperties>,
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
    ErasedPointCloudMaterialPipelineKey,
) -> Result<(), SpecializedMeshPipelineError>;
