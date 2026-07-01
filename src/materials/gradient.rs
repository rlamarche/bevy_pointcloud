use bevy::{
    asset::{Asset, Handle},
    color::{Color, ColorToComponents, LinearRgba},
    image::Image,
    math::Vec4,
    pbr::MeshPipelineKey,
    reflect::{std_traits::ReflectDefault, Reflect},
    render::{
        render_asset::RenderAssets,
        render_resource::{AsBindGroup, AsBindGroupShaderType, Face, ShaderType},
        texture::GpuImage,
    },
};
use bitflags::bitflags;

use crate::Material;

/// A material with "standard" properties used in PBR lighting.
/// Standard property values with pictures here:
/// <https://google.github.io/filament/notes/material_properties.html>.
///
/// May be created directly from a [`Color`] or an [`Image`].
///
/// The `StandardMaterial` can be extended with more data and custom
/// shaders using [`ExtendedMaterial`]. Examples of how to do this can
/// be found in the Bevy examples.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[bind_group_data(GradientPointCloudMaterialKey)]
#[data(0, GradientPointCloudMaterialUniform, binding_array(0))]
// #[bindless(index_table(range(0..31)))]
#[reflect(Default, Debug, Clone)]
pub struct GradientPointCloudMaterial {
    /// The start color of the gradient, defaults to [`Color::WHITE`]
    pub start_color: Color,
    /// The middle color of the gradient, defaults to [`Option::None`].
    /// If `None`, there will be no middle color in the gradient.
    pub middle_color: Option<Color>,
    /// The end color of the gradient, defaults to [`Color::WHITE`]
    pub end_color: Color,

    pub point_size: f32,

    /// Whether to cull the "front", "back" or neither side of a point.
    /// If set to `None`, the two sides of the point are visible.
    ///
    /// Defaults to `None`.
    // TODO: include this in reflection somehow (maybe via remote types like serde https://serde.rs/remote-derive.html)
    #[reflect(ignore, clone)]
    pub cull_mode: Option<Face>,

    /// The texture component of the material's color before lighting.
    /// The actual pre-lighting color is `base_color * this_texture`.
    ///
    /// See [`base_color`] for details.
    ///
    /// You should set `base_color` to [`Color::WHITE`] (the default)
    /// if you want the texture to show as-is.
    ///
    /// Setting `base_color` to something else than white will tint
    /// the texture. For example, setting `base_color` to pure red will
    /// tint the texture red.
    ///
    /// [`base_color`]: StandardMaterial::base_color
    #[texture(1)]
    #[sampler(2)]
    #[dependency]
    pub base_color_texture: Option<Handle<Image>>,
}

impl Default for GradientPointCloudMaterial {
    fn default() -> Self {
        GradientPointCloudMaterial {
            // White because it gets multiplied with texture values if someone uses
            // a texture.
            start_color: Color::WHITE,
            middle_color: None,
            end_color: Color::WHITE,
            point_size: 1.0,
            cull_mode: None,
            base_color_texture: None,
        }
    }
}

// NOTE: These must match the bit flags in bevy_pbr/src/render/pbr_types.wgsl!
bitflags::bitflags! {
    /// Bitflags info about the material a shader is currently rendering.
    /// This is accessible in the shader in the [`GradientPointCloudMaterialUniform`]
    #[repr(transparent)]
    pub struct GradientPointCloudMaterialFlags: u32 {
        const BASE_COLOR_TEXTURE         = 1 << 0;
        const NONE                       = 0;
        const UNINITIALIZED              = 0xFFFF;
    }
}

/// The GPU representation of the uniform data of a [`StandardMaterial`].
#[derive(Clone, Default, ShaderType)]
pub struct GradientPointCloudMaterialUniform {
    pub start_color: Vec4,
    pub middle_color: Vec4,
    pub end_color: Vec4,

    pub point_size: f32,

    /// The [`GradientPointCloudMaterialFlags`] accessible in the `wgsl` shader.
    pub flags: u32,
}

impl AsBindGroupShaderType<GradientPointCloudMaterialUniform> for GradientPointCloudMaterial {
    fn as_bind_group_shader_type(
        &self,
        _images: &RenderAssets<GpuImage>,
    ) -> GradientPointCloudMaterialUniform {
        let mut flags = GradientPointCloudMaterialFlags::NONE;
        if self.base_color_texture.is_some() {
            flags |= GradientPointCloudMaterialFlags::BASE_COLOR_TEXTURE;
        }

        GradientPointCloudMaterialUniform {
            start_color: LinearRgba::from(self.start_color).to_vec4(),
            middle_color: LinearRgba::from(self.middle_color.unwrap_or_default()).to_vec4(),
            end_color: LinearRgba::from(self.end_color).to_vec4(),
            point_size: self.point_size,
            flags: flags.bits(),
        }
    }
}

bitflags! {
    /// The pipeline key for `StandardMaterial`, packed into 64 bits.
    #[repr(C)]
    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    pub struct GradientPointCloudMaterialKey: u64 {
        const CULL_FRONT               = 0x000001;
        const CULL_BACK                = 0x000002;
        const MIDDLE_COLOR    = 0x000004;
    }
}

// const STANDARD_MATERIAL_KEY_DEPTH_BIAS_SHIFT: u64 = 32;

impl From<&GradientPointCloudMaterial> for GradientPointCloudMaterialKey {
    fn from(material: &GradientPointCloudMaterial) -> Self {
        let mut key = GradientPointCloudMaterialKey::empty();

        key.set(
            GradientPointCloudMaterialKey::CULL_FRONT,
            material.cull_mode == Some(Face::Front),
        );
        key.set(
            GradientPointCloudMaterialKey::CULL_BACK,
            material.cull_mode == Some(Face::Back),
        );
        key.set(
            GradientPointCloudMaterialKey::MIDDLE_COLOR,
            material.middle_color.is_some(),
        );

        key
    }
}

impl Material for GradientPointCloudMaterial {
    fn vertex_shader() -> bevy::shader::ShaderRef {
        "embedded://bevy_pointcloud/assets/shaders/pointcloud.wgsl".into()
    }
    fn fragment_shader() -> bevy::shader::ShaderRef {
        "embedded://bevy_pointcloud/assets/shaders/pointcloud.wgsl".into()
    }
    fn specialize(
        _pipeline: &crate::MaterialPipeline,
        descriptor: &mut bevy::material::descriptor::RenderPipelineDescriptor,
        _shape_layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _instance_layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: crate::MaterialPipelineKey<Self>,
    ) -> bevy::ecs::error::Result<(), bevy::material::specialize::SpecializedMeshPipelineError>
    {
        if let Some(fragment) = descriptor.fragment.as_mut() {
            let shader_defs = &mut fragment.shader_defs;

            for (flags, shader_def) in [(
                GradientPointCloudMaterialKey::MIDDLE_COLOR,
                "GRADIENT_MATERIAL_MIDDLE_COLOR",
            )] {
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
                .contains(GradientPointCloudMaterialKey::CULL_FRONT),
            key.bind_group_data
                .contains(GradientPointCloudMaterialKey::CULL_BACK),
            key.mesh_key.contains(MeshPipelineKey::INVERT_CULLING),
        ) {
            (true, false, false) | (false, true, true) => Some(Face::Front),
            (false, true, false) | (true, false, true) => Some(Face::Back),
            _ => None,
        };

        Ok(())
    }
}
