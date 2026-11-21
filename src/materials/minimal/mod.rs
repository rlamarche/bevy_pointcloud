use bevy::{
    app::Plugin,
    asset::{embedded_asset, Asset},
    color::{Color, ColorToComponents, LinearRgba},
    math::Vec4,
    pbr::MeshPipelineKey,
    reflect::{std_traits::ReflectDefault, Reflect},
    render::{
        render_asset::RenderAssets,
        render_resource::{
            AsBindGroup, AsBindGroupShaderType, BlendComponent, BlendFactor, BlendOperation,
            BlendState, Face, ShaderStages, ShaderType, TextureFormat,
        },
        texture::GpuImage,
    },
    shader::{load_shader_library, ShaderRef},
};
use bitflags::bitflags;
use strum::{EnumIter, FromRepr};

use crate::{
    render::{
        shader_ref, PassDescriptor, PassInput, PassKind, PassOutput, PassParams,
        PointCloudMaterial, PointCloudMaterialPipeline, PointCloudMaterialPipelineKey,
        PointCloudMaterialPlugin, TransientTarget,
    },
    PointCloudViewSettings, PointCloudViewSettingsKey, RenderMode, StandardPasses, ViewSettings,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, FromRepr, EnumIter)]
#[repr(usize)]
pub enum MinimalPasses {
    Depth = 0,
    Attribute = 1,
    EDLNormalize = 2,
}

impl TryFrom<usize> for MinimalPasses {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        MinimalPasses::from_repr(value).ok_or(())
    }
}

impl From<MinimalPasses> for usize {
    fn from(value: MinimalPasses) -> Self {
        value as usize
    }
}

pub struct MinimalPointCloudMaterialPlugin;

impl Plugin for MinimalPointCloudMaterialPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        load_shader_library!(app, "minimal_bindings.wgsl");
        load_shader_library!(app, "minimal_types.wgsl");
        embedded_asset!(app, "minimal.wgsl");
        embedded_asset!(app, "minimal_prepass.wgsl");
        embedded_asset!(app, "minimal_normalize.wgsl");

        app.add_plugins(PointCloudMaterialPlugin::<MinimalPointCloudMaterial>::default());
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
#[bind_group_data(MinimalPointCloudMaterialKey)]
#[data(0, MinimalPointCloudMaterialUniform, binding_array(0))]
// #[bindless(index_table(range(0..31)))]
#[reflect(Default, Debug, Clone)]
pub struct MinimalPointCloudMaterial {
    /// The color of the surface of the material before lighting.
    ///
    /// Doubles as diffuse albedo for non-metallic, specular for metallic and a mix for everything
    /// in between. If used together with a `base_color_texture`, this is factored into the final
    /// base color as `base_color * base_color_texture_value`.
    ///
    /// Defaults to [`Color::WHITE`].
    pub base_color: Color,

    pub render_mode: RenderMode,

    pub weighted_splats: bool,
}

impl Default for MinimalPointCloudMaterial {
    fn default() -> Self {
        MinimalPointCloudMaterial {
            // White because it gets multiplied with texture values if someone uses
            // a texture.
            base_color: Color::WHITE,
            render_mode: RenderMode::Normal,
            weighted_splats: false,
        }
    }
}

impl From<Color> for MinimalPointCloudMaterial {
    fn from(color: Color) -> Self {
        MinimalPointCloudMaterial {
            base_color: color,
            ..Default::default()
        }
    }
}

impl PointCloudMaterial for MinimalPointCloudMaterial {
    type PassId = MinimalPasses;
    type ViewSettings = PointCloudViewSettings;

    fn vertex_shader() -> ShaderRef {
        shader_ref(bevy::asset::embedded_path!("minimal.wgsl"))
    }

    fn fragment_shader() -> ShaderRef {
        shader_ref(bevy::asset::embedded_path!("minimal.wgsl"))
    }

    fn prepass_vertex_shader() -> ShaderRef {
        shader_ref(bevy::asset::embedded_path!("minimal.wgsl"))
    }

    fn prepass_fragment_shader() -> ShaderRef {
        shader_ref(bevy::asset::embedded_path!("minimal_prepass.wgsl"))
    }

    fn enable_prepass() -> bool {
        true
    }

    fn enable_shadows() -> bool {
        true
    }

    fn pass_descriptor(pass: &Self::PassId) -> PassDescriptor<Self::PassId> {
        match pass {
            MinimalPasses::Depth => PassDescriptor {
                id: MinimalPasses::Depth,
                kind: PassKind::Geometry,
                label: "depth",
            },
            MinimalPasses::Attribute => PassDescriptor {
                id: MinimalPasses::Attribute,
                kind: PassKind::Geometry,
                label: "attribute",
            },
            MinimalPasses::EDLNormalize => PassDescriptor {
                id: MinimalPasses::EDLNormalize,
                kind: PassKind::Fullscreen,
                label: "edl_normalize",
            },
        }
    }

    fn prepare_pass(
        pass: &Self::PassId,
        settings_key: &<Self::ViewSettings as ViewSettings>::Key,
    ) -> Option<PassParams> {
        if matches!(
            pass,
            MinimalPasses::Depth | MinimalPasses::Attribute | MinimalPasses::EDLNormalize
        ) && !settings_key.contains(PointCloudViewSettingsKey::RENDER_MODE_MULTIPASS)
        {
            return None;
        }

        match pass {
            MinimalPasses::Depth => Some(PassParams {
                vertex_shader: ShaderRef::Default,
                fragment_shader: ShaderRef::Default,
                inputs: vec![],
                output: PassOutput::TransientTarget(TransientTarget {
                    format: if settings_key.edl_enabled() {
                        TextureFormat::Rg16Float
                    } else {
                        TextureFormat::R16Float
                    },
                }),
                blend: None,
                depth_write_enabled: Some(true),
            }),
            MinimalPasses::Attribute => {
                Some(PassParams {
                    vertex_shader: ShaderRef::Default,
                    fragment_shader: ShaderRef::Default,
                    inputs: vec![],
                    output: PassOutput::TransientTarget(TransientTarget {
                        format: TextureFormat::Rgba32Float,
                    }),
                    blend: Some(BlendState {
                        color: BlendComponent {
                            // To match Potree blending
                            src_factor: BlendFactor::SrcAlpha,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent {
                            // To match Potree blending
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                    }),
                    depth_write_enabled: Some(false),
                })
            }
            MinimalPasses::EDLNormalize => Some(PassParams {
                vertex_shader: ShaderRef::Default,
                fragment_shader: shader_ref(bevy::asset::embedded_path!("minimal_normalize.wgsl")),
                inputs: vec![
                    PassInput {
                        source_pass: StandardPasses::Depth.into(),
                        binding_slot: 10,
                        texture_sample_type:
                            bevy::render::render_resource::TextureSampleType::Float {
                                filterable: false,
                            },
                        visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    },
                    {
                        PassInput {
                            source_pass: StandardPasses::Attribute.into(),
                            binding_slot: 11,
                            texture_sample_type:
                                bevy::render::render_resource::TextureSampleType::Float {
                                    filterable: false,
                                },
                            visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                        }
                    },
                ],
                output: PassOutput::MainColorTarget,
                blend: None,
                depth_write_enabled: Some(false),
            }),
        }
    }

    fn fullscreen_specialize(
        _pipeline: &crate::render::multipass::FullscreenMaterialPassPipeline,
        descriptor: &mut bevy::material::descriptor::RenderPipelineDescriptor,
        _pass: Self::PassId,
        settings_key: <Self::ViewSettings as ViewSettings>::Key,
    ) -> bevy::ecs::error::Result<(), bevy::material::specialize::SpecializedMeshPipelineError>
    {
        // Collect all shader defs for both vertex and fragment stages
        let mut shader_defs = Vec::new();

        // Evaluate single boolean flags
        #[expect(clippy::single_element_loop, reason = "todo")]
        for (flags, shader_def) in [(PointCloudViewSettingsKey::EDL_ENABLED, "RENDER_MODE_EDL")] {
            if settings_key.intersects(flags) {
                shader_defs.push(shader_def.into());
            }
        }

        // Forward shader defs to the vertex stage
        descriptor.vertex.shader_defs.extend(shader_defs.clone());

        // Forward shader defs to the fragment stage if it exists
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader_defs.extend(shader_defs);
        }

        Ok(())
    }

    fn specialize(
        _pipeline: &PointCloudMaterialPipeline,
        descriptor: &mut bevy::material::descriptor::RenderPipelineDescriptor,
        _splat_layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _instance_layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: PointCloudMaterialPipelineKey<Self>,
    ) -> bevy::ecs::error::Result<bool, bevy::material::specialize::SpecializedMeshPipelineError>
    {
        // Extract the custom material key from the bind group data
        let material_key = key.bind_group_data;

        // TODO: use this key to define a shaderdef with number of clipping planes
        let settings_key = key.view_settings_key;

        let render_mode = material_key.render_mode();
        let weighted_splats_enabled = material_key.weighted_splats_enabled();
        let edl_enabled = settings_key.edl_enabled();

        let should_render = match key.pass {
            None => {
                !matches!(render_mode, RenderMode::XRay) && !weighted_splats_enabled && !edl_enabled
            }
            Some(MinimalPasses::Depth) | Some(MinimalPasses::Attribute) => {
                matches!(render_mode, RenderMode::Normal)
                    && (weighted_splats_enabled || edl_enabled)
            }
            _ => panic!("Invalid pass in minimal material specialization."),
        };

        // Collect all shader defs for both vertex and fragment stages
        let mut shader_defs = Vec::new();

        // Evaluate single boolean flags
        for (flags, shader_def) in [
            (PointCloudViewSettingsKey::EDL_ENABLED, "RENDER_MODE_EDL"),
            (
                PointCloudViewSettingsKey::EDL_ENABLED,
                "POINTCLOUD_NEEDS_LOG_DEPTH",
            ),
            (
                PointCloudViewSettingsKey::CLIPPING_PLANES,
                "CLIPPING_PLANES",
            ),
            (
                PointCloudViewSettingsKey::CLIPPING_PLANES,
                "POINTCLOUD_NEEDS_WORLD_POSITION",
            ),
            (
                PointCloudViewSettingsKey::CLIPPING_SPHERES,
                "CLIPPING_SPHERES",
            ),
        ] {
            if settings_key.intersects(flags) {
                shader_defs.push(shader_def.into());
            }
        }

        // Evaluate multi-bit point size mode using the mask
        match material_key.render_mode() {
            RenderMode::Normal => {
                shader_defs.push("RENDER_MODE_NORMAL".into());
            }
            RenderMode::XRay => {
                shader_defs.push("RENDER_MODE_XRAY".into());
            }
        }

        // Evaluate single boolean flags
        for (flags, shader_def) in [
            (
                MinimalPointCloudMaterialKey::BASE_COLOR_TEXTURE,
                "POINTCLOUD_NEEDS_UVS",
            ),
            (
                MinimalPointCloudMaterialKey::WEIGHTED_SPLATS_ENABLED,
                "WEIGHTED_SPLATS_ENABLED",
            ),
            (
                MinimalPointCloudMaterialKey::GRADIENT,
                "MINIMAL_MATERIAL_GRADIENT",
            ),
            (
                MinimalPointCloudMaterialKey::GRADIENT,
                "POINTCLOUD_NEEDS_WORLD_POSITION",
            ),
        ] {
            if material_key.intersects(flags) {
                shader_defs.push(shader_def.into());
            }
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
            material_key.contains(MinimalPointCloudMaterialKey::CULL_FRONT),
            material_key.contains(MinimalPointCloudMaterialKey::CULL_BACK),
            key.mesh_key.contains(MeshPipelineKey::INVERT_CULLING),
        ) {
            (true, false, false) | (false, true, true) => Some(Face::Front),
            (false, true, false) | (true, false, true) => Some(Face::Back),
            _ => None,
        };

        // if `!should_render`, the prepass will still render, having all correct shaderdefs.
        Ok(should_render)
    }
}

// NOTE: These must match the bit flags in bevy_pbr/src/render/pbr_types.wgsl!
bitflags::bitflags! {
    /// Bitflags info about the material a shader is currently rendering.
    /// This is accessible in the shader in the [`MINIMALPointCloudMaterialUniform`]
    #[repr(transparent)]
    pub struct MinimalPointCloudMaterialFlags: u32 {
        const BASE_COLOR_TEXTURE         = 1 << 0;
        const NONE                       = 0;
        const UNINITIALIZED              = 0xFFFF;
    }
}

/// The GPU representation of the uniform data of a [`StandardMaterial`].
#[derive(Clone, Default, ShaderType)]
pub struct MinimalPointCloudMaterialUniform {
    /// Doubles as diffuse albedo for non-metallic, specular for metallic and a mix for everything
    /// in between.
    pub base_color: Vec4,

    /// The [`MinimalPointCloudMaterialFlags`] accessible in the `wgsl` shader.
    pub flags: u32,
}

impl AsBindGroupShaderType<MinimalPointCloudMaterialUniform> for MinimalPointCloudMaterial {
    fn as_bind_group_shader_type(
        &self,
        _images: &RenderAssets<GpuImage>,
    ) -> MinimalPointCloudMaterialUniform {
        let mut flags = MinimalPointCloudMaterialFlags::NONE;

        MinimalPointCloudMaterialUniform {
            base_color: LinearRgba::from(self.base_color).to_vec4(),
            flags: flags.bits(),
        }
    }
}

bitflags! {
    /// The pipeline key for `StandardMaterial`, packed into 64 bits.
    #[repr(C)]
    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    pub struct MinimalPointCloudMaterialKey: u64 {
        const CULL_FRONT               = 1_u64 <<  0;
        const CULL_BACK                = 1_u64 <<  1;
        const GRADIENT                 = 1_u64 <<  2;

        const RENDER_MODE_0            = 1_u64 <<  3;

        const RENDER_MODE_MASK          = Self::RENDER_MODE_0.bits();

        const RENDER_MODE_NORMAL       = 0;
        const RENDER_MODE_XRAY         = Self::RENDER_MODE_0.bits();

        const BASE_COLOR_TEXTURE       = 1_u64  <<  4;

        const WEIGHTED_SPLATS_ENABLED  = 1_u64  <<  5;
    }
}

impl MinimalPointCloudMaterialKey {
    pub fn render_mode(&self) -> RenderMode {
        match *self & MinimalPointCloudMaterialKey::RENDER_MODE_MASK {
            MinimalPointCloudMaterialKey::RENDER_MODE_NORMAL => RenderMode::Normal,
            MinimalPointCloudMaterialKey::RENDER_MODE_XRAY => RenderMode::XRay,
            _ => unreachable!("Invalid render mode bits state encountered in pipeline key."),
        }
    }
    pub fn weighted_splats_enabled(&self) -> bool {
        self.contains(MinimalPointCloudMaterialKey::WEIGHTED_SPLATS_ENABLED)
    }
}

// const STANDARD_MATERIAL_KEY_DEPTH_BIAS_SHIFT: u64 = 32;

impl From<&MinimalPointCloudMaterial> for MinimalPointCloudMaterialKey {
    fn from(material: &MinimalPointCloudMaterial) -> Self {
        let mut key = MinimalPointCloudMaterialKey::empty();

        key.set(
            MinimalPointCloudMaterialKey::RENDER_MODE_NORMAL,
            material.render_mode == RenderMode::Normal,
        );
        key.set(
            MinimalPointCloudMaterialKey::RENDER_MODE_XRAY,
            material.render_mode == RenderMode::XRay,
        );
        key.set(
            MinimalPointCloudMaterialKey::WEIGHTED_SPLATS_ENABLED,
            material.weighted_splats,
        );

        key
    }
}
