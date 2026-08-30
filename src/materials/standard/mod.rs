use bevy::{
    app::Plugin,
    asset::{embedded_asset, Asset, Handle},
    color::Color,
    image::Image,
    pbr::{StandardMaterial, StandardMaterialUniform},
    prelude::{Deref, DerefMut},
    reflect::{std_traits::ReflectDefault, Reflect},
    render::render_resource::{
        AsBindGroup, AsBindGroupShaderType, BindlessSlabResourceLimit, Face,
    },
    shader::ShaderRef,
};

use bevy::pbr::{Material as SourceMaterial, MeshPipelineKey, StandardMaterialKey};

use crate::{shader_ref, Material, MaterialPlugin};

pub struct StandardPointCloudMaterialPlugin;

impl Plugin for StandardPointCloudMaterialPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        embedded_asset!(app, "pbr.wgsl");
        embedded_asset!(app, "pbr_prepass.wgsl");

        app.add_plugins(MaterialPlugin::<StandardPointCloudMaterial>::default());
    }
}

#[derive(Asset, Debug, Clone, Default, Reflect, Deref, DerefMut)]
#[reflect(Default, Debug, Clone)]
pub struct StandardPointCloudMaterial(pub StandardMaterial);

/// Delegate implementation of [`AsBindGroup`] with bindless disabled.
impl AsBindGroup for StandardPointCloudMaterial {
    type Data = <StandardMaterial as AsBindGroup>::Data;

    type Param = <StandardMaterial as AsBindGroup>::Param;

    fn label() -> &'static str {
        "StandardPointCloudMaterial"
    }

    fn bind_group_data(&self) -> Self::Data {
        <StandardMaterial as AsBindGroup>::bind_group_data(&self.0)
    }

    fn unprepared_bind_group(
        &self,
        layout: &bevy::render::render_resource::BindGroupLayout,
        render_device: &bevy::render::renderer::RenderDevice,
        param: &mut bevy::ecs::system::SystemParamItem<'_, '_, Self::Param>,
        _force_no_bindless: bool,
    ) -> Result<
        bevy::render::render_resource::UnpreparedBindGroup,
        bevy::render::render_resource::AsBindGroupError,
    > {
        <StandardMaterial as AsBindGroup>::unprepared_bind_group(
            &self.0,
            layout,
            render_device,
            param,
            true,
        )
    }

    fn bind_group_layout_entries(
        render_device: &bevy::render::renderer::RenderDevice,
        _force_no_bindless: bool,
    ) -> Vec<bevy::render::render_resource::BindGroupLayoutEntry>
    where
        Self: Sized,
    {
        <StandardMaterial as AsBindGroup>::bind_group_layout_entries(render_device, true)
    }

    fn as_bind_group(
        &self,
        layout_descriptor: &bevy::material::descriptor::BindGroupLayoutDescriptor,
        render_device: &bevy::render::renderer::RenderDevice,
        pipeline_cache: &bevy::render::render_resource::PipelineCache,
        param: &mut bevy::ecs::system::SystemParamItem<'_, '_, Self::Param>,
    ) -> Result<
        bevy::render::render_resource::PreparedBindGroup,
        bevy::render::render_resource::AsBindGroupError,
    > {
        let layout = &pipeline_cache.get_bind_group_layout(layout_descriptor);

        let bevy::render::render_resource::UnpreparedBindGroup { bindings } =
            Self::unprepared_bind_group(self, layout, render_device, param, true)?;

        let entries = bindings
            .iter()
            .map(
                |(index, binding)| bevy::render::render_resource::BindGroupEntry {
                    binding: *index,
                    resource: binding.get_binding(),
                },
            )
            .collect::<Vec<_>>();

        let bind_group = render_device.create_bind_group(Self::label(), layout, &entries);

        Ok(bevy::render::render_resource::PreparedBindGroup {
            bindings,
            bind_group,
        })
    }

    fn bind_group_layout(
        render_device: &bevy::render::renderer::RenderDevice,
    ) -> bevy::render::render_resource::BindGroupLayout
    where
        Self: Sized,
    {
        render_device.create_bind_group_layout(
            Self::label(),
            &Self::bind_group_layout_entries(render_device, true),
        )
    }

    fn bind_group_layout_descriptor(
        render_device: &bevy::render::renderer::RenderDevice,
    ) -> bevy::material::descriptor::BindGroupLayoutDescriptor
    where
        Self: Sized,
    {
        bevy::material::descriptor::BindGroupLayoutDescriptor {
            label: Self::label().into(),
            entries: Self::bind_group_layout_entries(render_device, true),
        }
    }

    fn bindless_slot_count() -> Option<BindlessSlabResourceLimit> {
        None
    }

    fn bindless_supported(_: &bevy::render::renderer::RenderDevice) -> bool {
        false
    }

    fn bindless_descriptor() -> Option<bevy::render::render_resource::BindlessDescriptor> {
        None
    }
}

impl AsBindGroupShaderType<StandardMaterialUniform> for StandardPointCloudMaterial {
    fn as_bind_group_shader_type(
        &self,
        images: &bevy::render::render_asset::RenderAssets<bevy::render::texture::GpuImage>,
    ) -> StandardMaterialUniform {
        <StandardMaterial as AsBindGroupShaderType::<StandardMaterialUniform>>::as_bind_group_shader_type(&self.0, images)
    }
}

impl Material for StandardPointCloudMaterial {
    fn vertex_shader() -> ShaderRef {
        ShaderRef::Default
    }
    fn fragment_shader() -> ShaderRef {
        shader_ref(bevy::asset::embedded_path!("pbr.wgsl"))
        // "shaders/pbr_dev.wgsl".into()
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

    fn prepass_vertex_shader() -> ShaderRef {
        ShaderRef::Default
    }

    fn prepass_fragment_shader() -> ShaderRef {
        shader_ref(bevy::asset::embedded_path!("pbr_prepass.wgsl"))
        // "shaders/pbr_prepass_dev.wgsl".into()
    }

    fn deferred_vertex_shader() -> ShaderRef {
        ShaderRef::Default
    }

    fn deferred_fragment_shader() -> ShaderRef {
        shader_ref(bevy::asset::embedded_path!("pbr.wgsl"))
        // "shaders/pbr_dev.wgsl".into()
    }

    fn specialize(
        _pipeline: &crate::MaterialPipeline,
        descriptor: &mut bevy::material::descriptor::RenderPipelineDescriptor,
        _splat_layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _instance_layout: &bevy::mesh::MeshVertexBufferLayoutRef,
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
            *label = format!("pbr_pcl_{}", *label).into();
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

impl From<Color> for StandardPointCloudMaterial {
    fn from(value: Color) -> Self {
        Self(value.into())
    }
}

impl From<Handle<Image>> for StandardPointCloudMaterial {
    fn from(value: Handle<Image>) -> Self {
        Self(value.into())
    }
}

impl From<StandardMaterial> for StandardPointCloudMaterial {
    fn from(value: StandardMaterial) -> Self {
        Self(value)
    }
}
