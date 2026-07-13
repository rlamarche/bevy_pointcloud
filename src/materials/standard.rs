use bevy::{app::Plugin, pbr::StandardMaterial, render::render_resource::Face, shader::ShaderRef};

use bevy::pbr::{Material as SourceMaterial, MeshPipelineKey, StandardMaterialKey};

use crate::{Material, MaterialPlugin};

pub struct StandardPointCloudMaterialPlugin;

impl Plugin for StandardPointCloudMaterialPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_plugins(MaterialPlugin::<StandardMaterial>::default());
    }
}

impl Material for StandardMaterial {
    fn vertex_shader() -> ShaderRef {
        <StandardMaterial as SourceMaterial>::vertex_shader()
    }
    fn fragment_shader() -> ShaderRef {
        <StandardMaterial as SourceMaterial>::fragment_shader()
    }

    fn shape_mesh(&self) -> Option<bevy::asset::AssetId<bevy::mesh::Mesh>> {
        None
    }

    fn alpha_mode(&self) -> bevy::material::prelude::AlphaMode {
        <StandardMaterial as SourceMaterial>::alpha_mode(self)
    }

    fn opaque_render_method(&self) -> bevy::material::OpaqueRendererMethod {
        <StandardMaterial as SourceMaterial>::opaque_render_method(self)
    }

    fn depth_bias(&self) -> f32 {
        <StandardMaterial as SourceMaterial>::depth_bias(self)
    }

    fn reads_view_transmission_texture(&self) -> bool {
        <StandardMaterial as SourceMaterial>::reads_view_transmission_texture(self)
    }

    fn enable_prepass() -> bool {
        <StandardMaterial as SourceMaterial>::enable_prepass()
    }

    fn enable_shadows() -> bool {
        <StandardMaterial as SourceMaterial>::enable_shadows()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        <StandardMaterial as SourceMaterial>::prepass_vertex_shader()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        <StandardMaterial as SourceMaterial>::prepass_fragment_shader()
    }

    fn deferred_vertex_shader() -> ShaderRef {
        <StandardMaterial as SourceMaterial>::deferred_vertex_shader()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        <StandardMaterial as SourceMaterial>::deferred_fragment_shader()
    }

    fn specialize(
        pipeline: &crate::MaterialPipeline,
        descriptor: &mut bevy::material::descriptor::RenderPipelineDescriptor,
        shape_layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        instance_layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: crate::MaterialPipelineKey<Self>,
    ) -> bevy::ecs::error::Result<(), bevy::material::specialize::SpecializedMeshPipelineError>
    {
        if let Some(fragment) = descriptor.fragment.as_mut() {
            let shader_defs = &mut fragment.shader_defs;

            for (flags, shader_def) in [
                (
                    StandardMaterialKey::NORMAL_MAP,
                    "STANDARD_MATERIAL_NORMAL_MAP",
                ),
                (StandardMaterialKey::RELIEF_MAPPING, "RELIEF_MAPPING"),
                (
                    StandardMaterialKey::DIFFUSE_TRANSMISSION,
                    "STANDARD_MATERIAL_DIFFUSE_TRANSMISSION",
                ),
                (
                    StandardMaterialKey::SPECULAR_TRANSMISSION,
                    "STANDARD_MATERIAL_SPECULAR_TRANSMISSION",
                ),
                (
                    StandardMaterialKey::DIFFUSE_TRANSMISSION
                        | StandardMaterialKey::SPECULAR_TRANSMISSION,
                    "STANDARD_MATERIAL_DIFFUSE_OR_SPECULAR_TRANSMISSION",
                ),
                (
                    StandardMaterialKey::CLEARCOAT,
                    "STANDARD_MATERIAL_CLEARCOAT",
                ),
                (
                    StandardMaterialKey::CLEARCOAT_NORMAL_MAP,
                    "STANDARD_MATERIAL_CLEARCOAT_NORMAL_MAP",
                ),
                (
                    StandardMaterialKey::ANISOTROPY,
                    "STANDARD_MATERIAL_ANISOTROPY",
                ),
                (
                    StandardMaterialKey::BASE_COLOR_UV,
                    "STANDARD_MATERIAL_BASE_COLOR_UV_B",
                ),
                (
                    StandardMaterialKey::EMISSIVE_UV,
                    "STANDARD_MATERIAL_EMISSIVE_UV_B",
                ),
                (
                    StandardMaterialKey::METALLIC_ROUGHNESS_UV,
                    "STANDARD_MATERIAL_METALLIC_ROUGHNESS_UV_B",
                ),
                (
                    StandardMaterialKey::OCCLUSION_UV,
                    "STANDARD_MATERIAL_OCCLUSION_UV_B",
                ),
                (
                    StandardMaterialKey::SPECULAR_TRANSMISSION_UV,
                    "STANDARD_MATERIAL_SPECULAR_TRANSMISSION_UV_B",
                ),
                (
                    StandardMaterialKey::THICKNESS_UV,
                    "STANDARD_MATERIAL_THICKNESS_UV_B",
                ),
                (
                    StandardMaterialKey::DIFFUSE_TRANSMISSION_UV,
                    "STANDARD_MATERIAL_DIFFUSE_TRANSMISSION_UV_B",
                ),
                (
                    StandardMaterialKey::NORMAL_MAP_UV,
                    "STANDARD_MATERIAL_NORMAL_MAP_UV_B",
                ),
                (
                    StandardMaterialKey::CLEARCOAT_UV,
                    "STANDARD_MATERIAL_CLEARCOAT_UV_B",
                ),
                (
                    StandardMaterialKey::CLEARCOAT_ROUGHNESS_UV,
                    "STANDARD_MATERIAL_CLEARCOAT_ROUGHNESS_UV_B",
                ),
                (
                    StandardMaterialKey::CLEARCOAT_NORMAL_UV,
                    "STANDARD_MATERIAL_CLEARCOAT_NORMAL_UV_B",
                ),
                (
                    StandardMaterialKey::ANISOTROPY_UV,
                    "STANDARD_MATERIAL_ANISOTROPY_UV_B",
                ),
                (
                    StandardMaterialKey::SPECULAR_UV,
                    "STANDARD_MATERIAL_SPECULAR_UV_B",
                ),
                (
                    StandardMaterialKey::SPECULAR_TINT_UV,
                    "STANDARD_MATERIAL_SPECULAR_TINT_UV_B",
                ),
            ] {
                if key.bind_group_data.intersects(flags) {
                    shader_defs.push(shader_def.into());
                }
            }
        }

        // Generally, we want to cull front faces if `CULL_FRONT` is present and
        // backfaces if `CULL_BACK` is present. However, if the view has
        // `INVERT_CULLING` on (usually used for mirrors and the like), we do
        // the opposite.
        descriptor.primitive.cull_mode = match (
            key.bind_group_data
                .contains(StandardMaterialKey::CULL_FRONT),
            key.bind_group_data.contains(StandardMaterialKey::CULL_BACK),
            key.mesh_key.contains(MeshPipelineKey::INVERT_CULLING),
        ) {
            (true, false, false) | (false, true, true) => Some(Face::Front),
            (false, true, false) | (true, false, true) => Some(Face::Back),
            _ => None,
        };

        if let Some(label) = &mut descriptor.label {
            *label = format!("pbr_{}", *label).into();
        }
        if let Some(depth_stencil) = descriptor.depth_stencil.as_mut() {
            depth_stencil.bias.constant =
                (key.bind_group_data.bits() >> STANDARD_MATERIAL_KEY_DEPTH_BIAS_SHIFT) as i32;
        }
        Ok(())
    }
}

// Keep in sync with Bevy (private)
const STANDARD_MATERIAL_KEY_DEPTH_BIAS_SHIFT: u64 = 32;
