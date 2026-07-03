use bevy::{
    asset::{Asset, Handle},
    color::{Color, ColorToComponents, LinearRgba},
    image::Image,
    math::{Mat3, Vec2, Vec3, Vec4},
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

use crate::{ColorStop, ColorStopUniform, Material};

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

    /// A vec of color stops. Up to 8 color stops are currently supported.
    /// Empty by default.
    pub color_stops: Vec<ColorStop>,

    /// The end color, must be filled for the gradient to work.
    /// Defaults to `None`.
    pub end_color: Option<Color>,

    /// The gradient direction, must be filled for the gradient to work.
    /// Defaults to `None`.
    pub gradient_direction: Option<Vec3>,

    /// The gradient start bound (projected from world position), must be filled for the gradient
    /// to work.
    pub gradient_start: Option<f32>,

    /// The gradient end bound (projected from world position), must be filled for the gradient to
    /// work.
    pub gradient_end: Option<f32>,

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

    /// Determines the UV mapping type, when a [`SimplePointCloudMaterial::base_color_texture`] is
    /// used. See [`UVMapping`] documentation.
    pub uv_mapping: UVMapping,

    /// Determines an additionnal transformation to apply to UV coordinates obtained from the
    /// [`SimplePointCloudMaterial::uv_mapping`]
    pub uv_transform: Option<UVTransform>,
}

/// Determine the UV mapping coordinates mode when using a
/// [`SimplePointCloudMaterial::base_color_texture`].
#[derive(Reflect, Debug, Clone, Default)]
#[reflect(Default, Debug, Clone)]
pub enum UVMapping {
    #[default]
    /// Multiplies or adds Global UV (from point cloud data) and Local UV (from the point's shape).
    Combined,
    /// Uses UVs defined per vertex in the point cloud. The texture stretches across the whole
    /// cloud.
    PointCloudOnly,
    /// Uses UVs of the point's local geometry. The texture is repeated on every single point.
    ShapeOnly,
    /// Compute UV coordinates based on the provided [`SimplePointCloudMaterial::uv_direction`] and
    /// [`SimplePointCloudMaterial::uv_offset`]. The norm of the `uv_direction` provides the
    /// scaling.
    Planar,
}

impl UVMapping {
    /// Maps the UV mapping mode to its corresponding bits in `SimplePointCloudMaterialKey`.
    pub fn pipeline_key_bits(&self) -> SimplePointCloudMaterialKey {
        match self {
            Self::Combined => SimplePointCloudMaterialKey::UV_MAPPING_COMBINED,
            Self::PointCloudOnly => SimplePointCloudMaterialKey::UV_MAPPING_POINT_CLOUD,
            Self::ShapeOnly => SimplePointCloudMaterialKey::UV_MAPPING_POINT_SHAPE,
            Self::Planar => SimplePointCloudMaterialKey::UV_MAPPING_PLANAR,
        }
    }
}

#[derive(Reflect, Debug, Clone, Default)]
pub struct UVTransform {
    /// Décalage U et V (translation)
    pub offset: Vec2,
    /// Répétition / Échelle sur les axes U et V (Scale non-uniforme)
    pub scale: Vec2,
    /// Rotation en radians de la texture
    pub rotation: f32,
}

impl UVTransform {
    /// Computes 3x3 matrix from UV transform
    pub fn compute_matrix(&self) -> Mat3 {
        Mat3::from_translation(self.offset)
            * Mat3::from_angle(self.rotation)
            * Mat3::from_scale(self.scale)
    }
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
            color_stops: Vec::new(),
            gradient_direction: None,
            gradient_start: None,
            gradient_end: None,
            end_color: None,
            cull_mode: None,
            base_color_texture: None,
            uv_mapping: UVMapping::Combined,
            uv_transform: None,
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

    pub color_stops: [ColorStopUniform; 8],

    pub color_stop_count: u32,

    pub end_color: Vec4,

    pub gradient_direction: Vec3,

    pub gradient_start: f32,

    pub gradient_end: f32,

    pub uv_transform: Mat3,

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

        if self.color_stops.len() > 8 {
            panic!("Up to 8 color stops are supported in SimplePointCloudMaterial.");
        }

        SimplePointCloudMaterialUniform {
            base_color: LinearRgba::from(self.base_color).to_vec4(),
            point_size: self.point_size,
            shape_radius: self.shape_radius.unwrap_or_default(),
            color_stops: [
                #[expect(
                    clippy::get_first,
                    reason = "It is more idiomatic given the following lines."
                )]
                self.color_stops.get(0).cloned().unwrap_or_default().into(),
                self.color_stops.get(1).cloned().unwrap_or_default().into(),
                self.color_stops.get(2).cloned().unwrap_or_default().into(),
                self.color_stops.get(3).cloned().unwrap_or_default().into(),
                self.color_stops.get(4).cloned().unwrap_or_default().into(),
                self.color_stops.get(5).cloned().unwrap_or_default().into(),
                self.color_stops.get(6).cloned().unwrap_or_default().into(),
                self.color_stops.get(7).cloned().unwrap_or_default().into(),
            ],
            color_stop_count: self.color_stops.len() as u32,
            end_color: self
                .end_color
                .map(|c| LinearRgba::from(c).to_vec4())
                .unwrap_or_default(),
            gradient_direction: self.gradient_direction.unwrap_or_default(),
            gradient_start: self.gradient_start.unwrap_or_default(),
            gradient_end: self.gradient_end.unwrap_or_default(),
            flags: flags.bits(),
            uv_transform: self
                .uv_transform
                .clone()
                .unwrap_or_default()
                .compute_matrix(),
        }
    }
}

bitflags! {
    /// The pipeline key for `StandardMaterial`, packed into 64 bits.
    #[repr(C)]
    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SimplePointCloudMaterialKey: u64 {
        const CULL_FRONT               = 1_u64 <<  0;
        const CULL_BACK                = 1_u64 <<  1;
        const SHAPE_RADIUS             = 1_u64 <<  2;
        const GRADIENT                 = 1_u64 <<  3;

        const COLOR_STOP_1             = 1_u64 <<  4;
        const COLOR_STOP_2             = 1_u64 <<  5;
        const COLOR_STOP_3             = 1_u64 <<  6;
        const COLOR_STOP_4             = 1_u64 <<  7;
        const COLOR_STOP_5             = 1_u64 <<  8;
        const COLOR_STOP_6             = 1_u64 <<  9;
        const COLOR_STOP_7             = 1_u64 << 10;
        const COLOR_STOP_8             = 1_u64 << 11;

        const UV_MAPPING_BITS_0        = 1_u64 << 12;
        const UV_MAPPING_BITS_1        = 1_u64 << 13;

        const UV_TRANSFORM             = 1_u64 << 14;

        const UV_MAPPING_MASK          = Self::UV_MAPPING_BITS_0.bits() | Self::UV_MAPPING_BITS_1.bits();

        const UV_MAPPING_COMBINED      = 0;
        const UV_MAPPING_POINT_CLOUD   = Self::UV_MAPPING_BITS_0.bits();
        const UV_MAPPING_POINT_SHAPE   = Self::UV_MAPPING_BITS_1.bits();
        const UV_MAPPING_PLANAR        = Self::UV_MAPPING_BITS_0.bits() | Self::UV_MAPPING_BITS_1.bits();
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
        key.set(
            SimplePointCloudMaterialKey::GRADIENT,
            material.end_color.is_some()
                && material.gradient_direction.is_some()
                && material.gradient_start.is_some()
                && material.gradient_end.is_some(),
        );
        key.set(
            SimplePointCloudMaterialKey::COLOR_STOP_1,
            !material.color_stops.is_empty(),
        );
        key.set(
            SimplePointCloudMaterialKey::COLOR_STOP_2,
            material.color_stops.len() >= 2,
        );
        key.set(
            SimplePointCloudMaterialKey::COLOR_STOP_3,
            material.color_stops.len() >= 3,
        );
        key.set(
            SimplePointCloudMaterialKey::COLOR_STOP_4,
            material.color_stops.len() >= 4,
        );
        key.set(
            SimplePointCloudMaterialKey::COLOR_STOP_5,
            material.color_stops.len() >= 5,
        );
        key.set(
            SimplePointCloudMaterialKey::COLOR_STOP_6,
            material.color_stops.len() >= 6,
        );
        key.set(
            SimplePointCloudMaterialKey::COLOR_STOP_7,
            material.color_stops.len() >= 7,
        );
        key.set(
            SimplePointCloudMaterialKey::COLOR_STOP_8,
            material.color_stops.len() >= 8,
        );

        key.insert(material.uv_mapping.pipeline_key_bits());

        key.set(
            SimplePointCloudMaterialKey::UV_TRANSFORM,
            material.uv_transform.is_some(),
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
        // Extract the custom material key from the bind group data
        let material_key = key.bind_group_data;

        // Collect all shader defs for both vertex and fragment stages
        let mut shader_defs = Vec::new();

        // 1. Evaluate single boolean flags
        for (flags, shader_def) in [
            (
                SimplePointCloudMaterialKey::SHAPE_RADIUS,
                "SIMPLE_MATERIAL_SHAPE_RADIUS",
            ),
            (
                SimplePointCloudMaterialKey::GRADIENT,
                "SIMPLE_MATERIAL_GRADIENT",
            ),
            (
                SimplePointCloudMaterialKey::COLOR_STOP_1,
                "SIMPLE_MATERIAL_COLOR_STOP_1",
            ),
            (
                SimplePointCloudMaterialKey::COLOR_STOP_2,
                "SIMPLE_MATERIAL_COLOR_STOP_2",
            ),
            (
                SimplePointCloudMaterialKey::COLOR_STOP_3,
                "SIMPLE_MATERIAL_COLOR_STOP_3",
            ),
            (
                SimplePointCloudMaterialKey::COLOR_STOP_4,
                "SIMPLE_MATERIAL_COLOR_STOP_4",
            ),
            (
                SimplePointCloudMaterialKey::COLOR_STOP_5,
                "SIMPLE_MATERIAL_COLOR_STOP_5",
            ),
            (
                SimplePointCloudMaterialKey::COLOR_STOP_6,
                "SIMPLE_MATERIAL_COLOR_STOP_6",
            ),
            (
                SimplePointCloudMaterialKey::COLOR_STOP_7,
                "SIMPLE_MATERIAL_COLOR_STOP_7",
            ),
            (
                SimplePointCloudMaterialKey::COLOR_STOP_8,
                "SIMPLE_MATERIAL_COLOR_STOP_8",
            ),
            (
                SimplePointCloudMaterialKey::UV_TRANSFORM,
                "SIMPLE_MATERIAL_HAS_UV_TRANSFORM",
            ),
        ] {
            if material_key.intersects(flags) {
                shader_defs.push(shader_def.into());
            }
        }

        // 2. Evaluate multi-bit UV mapping mode using the mask
        match material_key & SimplePointCloudMaterialKey::UV_MAPPING_MASK {
            SimplePointCloudMaterialKey::UV_MAPPING_COMBINED => {
                shader_defs.push("SIMPLE_MATERIAL_UV_MAPPING_COMBINED".into());
            }
            SimplePointCloudMaterialKey::UV_MAPPING_POINT_CLOUD => {
                shader_defs.push("SIMPLE_MATERIAL_UV_MAPPING_POINT_CLOUD".into());
            }
            SimplePointCloudMaterialKey::UV_MAPPING_POINT_SHAPE => {
                shader_defs.push("SIMPLE_MATERIAL_UV_MAPPING_POINT_SHAPE".into());
            }
            SimplePointCloudMaterialKey::UV_MAPPING_PLANAR => {
                shader_defs.push("SIMPLE_MATERIAL_UV_MAPPING_PLANAR".into());
            }
            _ => unreachable!("Invalid UV mapping bits state encountered in pipeline key."),
        }

        // Forward shader defs to the vertex stage
        descriptor.vertex.shader_defs.extend(shader_defs.clone());

        // Forward shader defs to the fragment stage if it exists
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader_defs.extend(shader_defs);
        }

        // Generally, we want to cull front faces if `CULL_FRONT` is present and
        // backfaces if `CULL_BACK` is present. However, if the view has
        // `INVERT_CULLING` on (usually used for mirrors and the like), we do
        // the opposite.
        descriptor.primitive.cull_mode = match (
            material_key.contains(SimplePointCloudMaterialKey::CULL_FRONT),
            material_key.contains(SimplePointCloudMaterialKey::CULL_BACK),
            key.mesh_key.contains(MeshPipelineKey::INVERT_CULLING),
        ) {
            (true, false, false) | (false, true, true) => Some(Face::Front),
            (false, true, false) | (true, false, true) => Some(Face::Back),
            _ => None,
        };

        Ok(())
    }
}
