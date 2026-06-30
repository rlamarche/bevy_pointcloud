use bevy::{
    asset::{Asset, Handle},
    color::{Color, ColorToComponents, LinearRgba},
    image::Image,
    math::Vec4,
    reflect::{std_traits::ReflectDefault, Reflect},
    render::{
        render_asset::RenderAssets,
        render_resource::{AsBindGroup, AsBindGroupShaderType, ShaderType},
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
#[bind_group_data(StandardPointCloudMaterialKey)]
#[data(0, StandardPointCloudMaterialUniform, binding_array(10))]
#[bindless(index_table(range(0..31)))]
#[reflect(Default, Debug, Clone)]
pub struct StandardPointCloudMaterial {
    /// The color of the surface of the material before lighting.
    ///
    /// Doubles as diffuse albedo for non-metallic, specular for metallic and a mix for everything
    /// in between. If used together with a `base_color_texture`, this is factored into the final
    /// base color as `base_color * base_color_texture_value`.
    ///
    /// Defaults to [`Color::WHITE`].
    pub base_color: Color,

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

impl Default for StandardPointCloudMaterial {
    fn default() -> Self {
        StandardPointCloudMaterial {
            // White because it gets multiplied with texture values if someone uses
            // a texture.
            base_color: Color::WHITE,
            base_color_texture: None,
        }
    }
}

/// The GPU representation of the uniform data of a [`StandardMaterial`].
#[derive(Clone, Default, ShaderType)]
pub struct StandardPointCloudMaterialUniform {
    /// Doubles as diffuse albedo for non-metallic, specular for metallic and a mix for everything
    /// in between.
    pub base_color: Vec4,
}

impl AsBindGroupShaderType<StandardPointCloudMaterialUniform> for StandardPointCloudMaterial {
    fn as_bind_group_shader_type(
        &self,
        _images: &RenderAssets<GpuImage>,
    ) -> StandardPointCloudMaterialUniform {
        StandardPointCloudMaterialUniform {
            base_color: LinearRgba::from(self.base_color).to_vec4(),
        }
    }
}

bitflags! {
    /// The pipeline key for `StandardMaterial`, packed into 64 bits.
    #[repr(C)]
    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    pub struct StandardPointCloudMaterialKey: u64 {
        const CULL_FRONT               = 0x000001;
        const CULL_BACK                = 0x000002;
        const NORMAL_MAP               = 0x000004;
        const RELIEF_MAPPING           = 0x000008;
        const DIFFUSE_TRANSMISSION     = 0x000010;
        const SPECULAR_TRANSMISSION    = 0x000020;
        const CLEARCOAT                = 0x000040;
        const CLEARCOAT_NORMAL_MAP     = 0x000080;
        const ANISOTROPY               = 0x000100;
        const BASE_COLOR_UV            = 0x000200;
        const EMISSIVE_UV              = 0x000400;
        const METALLIC_ROUGHNESS_UV    = 0x000800;
        const OCCLUSION_UV             = 0x001000;
        const SPECULAR_TRANSMISSION_UV = 0x002000;
        const THICKNESS_UV             = 0x004000;
        const DIFFUSE_TRANSMISSION_UV  = 0x008000;
        const NORMAL_MAP_UV            = 0x010000;
        const ANISOTROPY_UV            = 0x020000;
        const CLEARCOAT_UV             = 0x040000;
        const CLEARCOAT_ROUGHNESS_UV   = 0x080000;
        const CLEARCOAT_NORMAL_UV      = 0x100000;
        const SPECULAR_UV              = 0x200000;
        const SPECULAR_TINT_UV         = 0x400000;
        const DEPTH_BIAS               = 0xffffffff_00000000;
    }
}

// const STANDARD_MATERIAL_KEY_DEPTH_BIAS_SHIFT: u64 = 32;

impl From<&StandardPointCloudMaterial> for StandardPointCloudMaterialKey {
    fn from(_material: &StandardPointCloudMaterial) -> Self {
        let key = StandardPointCloudMaterialKey::empty();

        // TODO add here fields that changes the render phases / pipeline, ... (eg: blending)

        key
    }
}

impl Material for StandardPointCloudMaterial {
    fn vertex_shader() -> bevy::shader::ShaderRef {
        "embedded://bevy_pointcloud/assets/shaders/pointcloud.wgsl".into()
    }
    fn fragment_shader() -> bevy::shader::ShaderRef {
        "embedded://bevy_pointcloud/assets/shaders/pointcloud.wgsl".into()
    }
}
