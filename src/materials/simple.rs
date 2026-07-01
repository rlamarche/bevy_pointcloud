use bevy::{
    asset::{Asset, Handle},
    color::{Color, ColorToComponents, LinearRgba},
    image::Image,
    math::Vec4,
    mesh::Mesh,
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
#[bind_group_data(SimplePointCloudMaterialKey)]
#[data(0, SimplePointCloudMaterialUniform, binding_array(0))]
// #[bindless(index_table(range(0..31)))]
#[reflect(Default, Debug, Clone)]
pub struct SimplePointCloudMaterial {
    /// The shape radius, default to None.
    /// Put `Some(0.5)` for a perfect circle.
    /// If set, the points will be truncated to a circle radius of 1
    pub shape_radius: Option<f32>,

    /// The shape mesh. If `None`, will be a quad or triangle.
    /// Defaults to `None`.
    pub shape_mesh: Option<Handle<Mesh>>,

    /// The color of the surface of the material before lighting.
    ///
    /// Doubles as diffuse albedo for non-metallic, specular for metallic and a mix for everything
    /// in between. If used together with a `base_color_texture`, this is factored into the final
    /// base color as `base_color * base_color_texture_value`.
    ///
    /// Defaults to [`Color::WHITE`].
    pub base_color: Color,

    /// The point size world space dimensions.
    /// Using orthographic projection, the size will always match this size.
    /// Using perspective projection, the point size will fade with distance.
    /// Defaults to `0.01`.
    /// Note: the transform scale is applied to the point size.
    pub point_size: f32,

    /// Whether to cull the "front", "back" or neither side of a point.
    /// If set to `None`, the two sides of the point are visible.
    ///
    /// Defaults to `None`.
    /// Note: you need normals for this culling to work properly.
    /// Without normals, the point will always be visible.
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

impl Default for SimplePointCloudMaterial {
    fn default() -> Self {
        SimplePointCloudMaterial {
            shape_mesh: None,
            shape_radius: None,
            // White because it gets multiplied with texture values if someone uses
            // a texture.
            base_color: Color::WHITE,
            point_size: 0.01,
            cull_mode: None,
            base_color_texture: None,
        }
    }
}

// NOTE: These must match the bit flags in bevy_pbr/src/render/pbr_types.wgsl!
bitflags::bitflags! {
    /// Bitflags info about the material a shader is currently rendering.
    /// This is accessible in the shader in the [`SimplePointCloudMaterialUniform`]
    #[repr(transparent)]
    pub struct SimplePointCloudMaterialFlags: u32 {
        const BASE_COLOR_TEXTURE         = 1 << 0;
        const NONE                       = 0;
        const UNINITIALIZED              = 0xFFFF;
    }
}

/// The GPU representation of the uniform data of a [`StandardMaterial`].
#[derive(Clone, Default, ShaderType)]
pub struct SimplePointCloudMaterialUniform {
    /// Doubles as diffuse albedo for non-metallic, specular for metallic and a mix for everything
    /// in between.
    pub base_color: Vec4,

    pub point_size: f32,

    pub shape_radius: f32,

    /// The [`SimplePointCloudMaterialFlags`] accessible in the `wgsl` shader.
    pub flags: u32,
}

impl AsBindGroupShaderType<SimplePointCloudMaterialUniform> for SimplePointCloudMaterial {
    fn as_bind_group_shader_type(
        &self,
        _images: &RenderAssets<GpuImage>,
    ) -> SimplePointCloudMaterialUniform {
        let mut flags = SimplePointCloudMaterialFlags::NONE;
        if self.base_color_texture.is_some() {
            flags |= SimplePointCloudMaterialFlags::BASE_COLOR_TEXTURE;
        }

        SimplePointCloudMaterialUniform {
            base_color: LinearRgba::from(self.base_color).to_vec4(),
            point_size: self.point_size,
            shape_radius: self.shape_radius.unwrap_or_default(),
            flags: flags.bits(),
        }
    }
}

bitflags! {
    /// The pipeline key for `StandardMaterial`, packed into 64 bits.
    #[repr(C)]
    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SimplePointCloudMaterialKey: u64 {
        const CULL_FRONT               = 0x000001;
        const CULL_BACK                = 0x000002;
        const SHAPE_RADIUS             = 0x000004;
    }
}

// const STANDARD_MATERIAL_KEY_DEPTH_BIAS_SHIFT: u64 = 32;

impl From<&SimplePointCloudMaterial> for SimplePointCloudMaterialKey {
    fn from(material: &SimplePointCloudMaterial) -> Self {
        let mut key = SimplePointCloudMaterialKey::empty();

        key.set(
            SimplePointCloudMaterialKey::CULL_FRONT,
            material.cull_mode == Some(Face::Front),
        );
        key.set(
            SimplePointCloudMaterialKey::CULL_BACK,
            material.cull_mode == Some(Face::Back),
        );
        key.set(
            SimplePointCloudMaterialKey::SHAPE_RADIUS,
            material.shape_radius.is_some(),
        );

        key
    }
}

impl Material for SimplePointCloudMaterial {
    fn vertex_shader() -> bevy::shader::ShaderRef {
        "embedded://bevy_pointcloud/assets/shaders/pointcloud.wgsl".into()
    }
    fn fragment_shader() -> bevy::shader::ShaderRef {
        "embedded://bevy_pointcloud/assets/shaders/pointcloud.wgsl".into()
    }
    fn shape_mesh(&self) -> Option<bevy::asset::AssetId<Mesh>> {
        self.shape_mesh.as_ref().map(Handle::id)
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
                SimplePointCloudMaterialKey::SHAPE_RADIUS,
                "SIMPLE_MATERIAL_SHAPE_RADIUS",
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
                .contains(SimplePointCloudMaterialKey::CULL_FRONT),
            key.bind_group_data
                .contains(SimplePointCloudMaterialKey::CULL_BACK),
            key.mesh_key.contains(MeshPipelineKey::INVERT_CULLING),
        ) {
            (true, false, false) | (false, true, true) => Some(Face::Front),
            (false, true, false) | (true, false, true) => Some(Face::Back),
            _ => None,
        };

        Ok(())
    }
}
