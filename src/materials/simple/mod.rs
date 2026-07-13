use bevy::{
    app::Plugin,
    asset::{embedded_asset, Asset, Handle},
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

use crate::{shader_ref, ColorStop, ColorStopUniform, Material, MaterialPlugin};

pub struct SimplePointCloudMaterialPlugin;

impl Plugin for SimplePointCloudMaterialPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        embedded_asset!(app, "simple.wgsl");

        app.add_plugins(MaterialPlugin::<SimplePointCloudMaterial>::default());
    }
}

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

    /// Determines the shapes orientation. See [`ShapeOrientation`]
    /// documentation for options.
    pub shape_orientation: ShapeOrientation,

    /// The default normal (if missing on the vertex attributes).
    /// Used for the shape orientation in [`ShapeOrientation::FaceNormal`] mode.
    pub default_normal: Vec3,

    /// The color of the surface of the material before lighting.
    ///
    /// Doubles as diffuse albedo for non-metallic, specular for metallic and a mix for everything
    /// in between. If used together with a `base_color_texture`, this is factored into the final
    /// base color as `base_color * base_color_texture_value`.
    ///
    /// Defaults to [`Color::WHITE`].
    pub base_color: Color,

    pub point_size_mode: PointSizeMode,

    /// The point size in pixels.
    /// Using orthographic projection, the size in pixels will always match this size.
    /// Using perspective projection, the point size will fade with distance, and grow.
    /// Defaults to `0.01`.
    /// Note: the transform scale is applied to the point size.
    pub point_size: f32,

    pub min_point_size: Option<f32>,
    pub max_point_size: Option<f32>,

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

    /// The UV U vector on which the point position, in object space, will be projected to compute
    /// U when using a computed UV mapping. Computed UV mapping are:
    /// - [`UVMapping::Planar`]
    /// - TODO: add more (spherical, cylindrical, ...)
    pub uv_u: Vec3,

    /// The UV V vector on which the point position, in object space, will be projected to compute
    /// V when using a computed UV mapping. Computed UV mapping are:
    /// - [`UVMapping::Planar`]
    /// - TODO: add more (spherical, cylindrical, ...)
    pub uv_v: Vec3,

    /// Determines an additionnal transformation to apply to UV coordinates obtained from the
    /// [`SimplePointCloudMaterial::uv_mapping`]
    pub uv_transform: Option<UVTransform>,
}

/// Determines how the point size is interpreted.
#[derive(Reflect, Debug, Clone, Default)]
#[reflect(Default, Debug, Clone)]
pub enum PointSizeMode {
    /// Point size is specified in screen pixels.
    ///
    /// - In [`bevy::camera::Projection::Perspective`] mode, points will appear smaller as they get
    ///   further away from the camera (perspective divide).
    /// - In [`bevy::camera::Projection::Orthographic`] mode, points will maintain a constant pixel
    ///   size regardless of the camera's distance or zoom level.
    ScreenPixels,

    /// Point size is specified in screen pixels, relative to the entity's transform.
    ///
    /// Similar to [`PointSizeMode::ScreenPixels`], but the final size is multiplied by the
    /// entity's [`bevy::transform::components::Transform`] scale. If you scale the entity,
    /// the points will scale accordingly. If the scaling is not the same on all axes, the max
    /// scale will be retained.
    #[default]
    ScreenPixelsLocal,

    /// Point size is specified in world-space units (e.g., meters).
    ///
    /// The size is absolute within the 3D world and is unaffected by the entity's
    /// [`bevy::transform::components::Transform`] scale. Points will correctly scale
    /// with camera distance, zoom, and projection modes (both Perspective and Orthographic).
    WorldSpace,

    /// Point size is specified in local-space units, relative to the entity's transform.
    ///
    /// Similar to [`PointSizeMode::WorldSpace`], but the final size is multiplied by the
    /// entity's [`bevy::transform::components::Transform`] scale. If you scale the entity,
    /// the points will scale accordingly. If the scaling is not the same on all axes, the max
    /// scale will be retained.
    LocalSpace,
}

impl PointSizeMode {
    /// Maps the UV mapping mode to its corresponding bits in `SimplePointCloudMaterialKey`.
    pub fn pipeline_key_bits(&self) -> SimplePointCloudMaterialKey {
        match self {
            PointSizeMode::ScreenPixels => SimplePointCloudMaterialKey::POINT_SIZE_SCREEN,
            PointSizeMode::ScreenPixelsLocal => {
                SimplePointCloudMaterialKey::POINT_SIZE_SCREEN_LOCAL
            }
            PointSizeMode::WorldSpace => SimplePointCloudMaterialKey::POINT_SIZE_WORLD,
            PointSizeMode::LocalSpace => SimplePointCloudMaterialKey::POINT_SIZE_LOCAL,
        }
    }
}

/// Determines the shape orientation.
#[derive(Reflect, Debug, Clone, Default)]
#[reflect(Default, Debug, Clone)]
pub enum ShapeOrientation {
    /// The shape always faces the camera (classic billboard).
    #[default]
    Billboard,
    /// The shape is oriented along the point's normal vector.
    /// Works only if a normal is provided, and works better if also a tangent is provided.
    FaceNormal,
}

/// Determine the UV mapping coordinates mode when using a
/// [`SimplePointCloudMaterial::base_color_texture`].
#[derive(Reflect, Debug, Clone, Default)]
#[reflect(Default, Debug, Clone)]
pub enum UVMapping {
    /// Combine global UV (from point cloud data) and Local UV (from the point's shape).
    /// **Needs stabilization.**
    Combined,
    /// Uses UVs defined per vertex in the point cloud. The texture stretches across the whole
    /// cloud and is pixelated. Needs the point cloud's UVs.
    PointCloudOnly,
    /// Uses UVs of the point's local geometry. The texture is repeated on every single point using
    /// shape's UV.
    #[default]
    ShapeOnly,
    /// Compute UV coordinates based on the provided [`SimplePointCloudMaterial::uv_u`] and
    /// [`SimplePointCloudMaterial::uv_v`].
    /// **Needs stabilization:**
    ///  * in billboard mode,the texture on each point does not follow the rotation of the view
    ///  * in face normal mode, the U/V projection vectors must match the UV coordinates of the
    ///    shape
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
            shape_orientation: ShapeOrientation::Billboard,
            default_normal: Vec3::Z,
            // White because it gets multiplied with texture values if someone uses
            // a texture.
            base_color: Color::WHITE,
            point_size_mode: PointSizeMode::ScreenPixelsLocal,
            point_size: 30.0,
            min_point_size: None,
            max_point_size: None,
            color_stops: Vec::new(),
            gradient_direction: None,
            gradient_start: None,
            gradient_end: None,
            end_color: None,
            cull_mode: None,
            base_color_texture: None,
            uv_mapping: UVMapping::Combined,
            uv_u: Vec3::X,
            uv_v: Vec3::Y,
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

    pub default_normal: Vec3,

    pub point_size: f32,

    pub min_point_size: f32,

    pub max_point_size: f32,

    pub shape_radius: f32,

    pub gradient_start: f32,

    pub gradient_end: f32,

    pub end_color: Vec4,

    pub color_stops: [ColorStopUniform; 8],

    pub gradient_direction: Vec3,

    pub uv_transform: Mat3,

    pub uv_u: Vec3,

    pub uv_v: Vec3,

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
            default_normal: self.default_normal,
            min_point_size: self.min_point_size.unwrap_or_default(),
            max_point_size: self.max_point_size.unwrap_or_default(),
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
            uv_u: self.uv_u,
            uv_v: self.uv_v,
        }
    }
}

bitflags! {
    /// The pipeline key for `StandardMaterial`, packed into 64 bits.
    #[repr(C)]
    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SimplePointCloudMaterialKey: u64 {
        const POINT_SIZE_MODE_BITS_0        = 1_u64 << 0;
        const POINT_SIZE_MODE_BITS_1        = 1_u64 << 1;

        const POINT_SIZE_MASK          = Self::POINT_SIZE_MODE_BITS_0.bits() | Self::POINT_SIZE_MODE_BITS_1.bits();

        const POINT_SIZE_SCREEN        = 0;
        const POINT_SIZE_SCREEN_LOCAL  = Self::POINT_SIZE_MODE_BITS_0.bits();
        const POINT_SIZE_WORLD         = Self::POINT_SIZE_MODE_BITS_1.bits();
        const POINT_SIZE_LOCAL         = Self::POINT_SIZE_MODE_BITS_0.bits() | Self::POINT_SIZE_MODE_BITS_1.bits();

        const CULL_FRONT               = 1_u64 <<  2;
        const CULL_BACK                = 1_u64 <<  3;
        const SHAPE_RADIUS             = 1_u64 <<  4;
        const SHAPE_ORIENTATION        = 1_u64 <<  5;
        const GRADIENT                 = 1_u64 <<  6;

        const COLOR_STOP_1             = 1_u64 <<  7;
        const COLOR_STOP_2             = 1_u64 <<  8;
        const COLOR_STOP_3             = 1_u64 <<  9;
        const COLOR_STOP_4             = 1_u64 <<  10;
        const COLOR_STOP_5             = 1_u64 <<  11;
        const COLOR_STOP_6             = 1_u64 <<  12;
        const COLOR_STOP_7             = 1_u64 <<  13;
        const COLOR_STOP_8             = 1_u64 <<  14;

        const UV_MAPPING_BITS_0        = 1_u64 <<  15;
        const UV_MAPPING_BITS_1        = 1_u64 <<  16;

        const UV_TRANSFORM             = 1_u64 <<  17;

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

        key.insert(material.point_size_mode.pipeline_key_bits());

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
            SimplePointCloudMaterialKey::SHAPE_ORIENTATION,
            matches!(material.shape_orientation, ShapeOrientation::FaceNormal),
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
        shader_ref(bevy::asset::embedded_path!("simple.wgsl"))
        // "shaders/pointcloud_dev.wgsl".into()
        // "embedded://bevy_pointcloud/assets/shaders/pointcloud.wgsl".into()
    }
    fn fragment_shader() -> bevy::shader::ShaderRef {
        shader_ref(bevy::asset::embedded_path!("simple.wgsl"))
        // "shaders/pointcloud_dev.wgsl".into()
        // "embedded://bevy_pointcloud/assets/shaders/pointcloud.wgsl".into()
    }
    fn shape_mesh(&self) -> Option<bevy::asset::AssetId<Mesh>> {
        self.shape_mesh.as_ref().map(Handle::id)
    }

    fn enable_shadows() -> bool {
        true
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

        // Evaluate multi-bit point size mode using the mask
        match material_key & SimplePointCloudMaterialKey::POINT_SIZE_MASK {
            SimplePointCloudMaterialKey::POINT_SIZE_SCREEN => {
                shader_defs.push("SIMPLE_MATERIAL_POINT_SIZE_SCREEN".into());
            }
            SimplePointCloudMaterialKey::POINT_SIZE_SCREEN_LOCAL => {
                shader_defs.push("SIMPLE_MATERIAL_UV_POINT_SIZE_SCREEN_LOCAL".into());
            }
            SimplePointCloudMaterialKey::POINT_SIZE_WORLD => {
                shader_defs.push("SIMPLE_MATERIAL_POINT_SIZE_WORLD".into());
            }
            SimplePointCloudMaterialKey::POINT_SIZE_LOCAL => {
                shader_defs.push("SIMPLE_MATERIAL_POINT_SIZE_LOCAL".into());
            }
            _ => unreachable!("Invalid UV mapping bits state encountered in pipeline key."),
        }

        // Evaluate single boolean flags
        for (flags, shader_def) in [
            (
                SimplePointCloudMaterialKey::SHAPE_RADIUS,
                "SIMPLE_MATERIAL_SHAPE_RADIUS",
            ),
            (
                SimplePointCloudMaterialKey::SHAPE_ORIENTATION,
                "SIMPLE_MATERIAL_SHAPE_ORIENTATION_FACE_NORMAL",
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

        // Evaluate multi-bit UV mapping mode using the mask
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
