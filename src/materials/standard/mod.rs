use bevy::{
    app::Plugin,
    asset::{embedded_asset, Asset, Handle},
    color::Color,
    image::Image,
    log::info,
    pbr::{StandardMaterial, StandardMaterialUniform},
    prelude::{Deref, DerefMut},
    reflect::{std_traits::ReflectDefault, Reflect},
    render::{
        render_asset::RenderAssets,
        render_resource::{
            AsBindGroup, AsBindGroupShaderType, BindlessSlabResourceLimit, BlendComponent,
            BlendFactor, BlendOperation, BlendState, Face, ShaderStages, TextureFormat,
        },
        texture::GpuImage,
    },
    shader::ShaderRef,
    utils::default,
};

use bevy::pbr::{Material as SourceMaterial, MeshPipelineKey, StandardMaterialKey};
use bitflags::bitflags;

use crate::{
    render::{
        shader_ref, PassDescriptor, PassInput, PassKind, PassOutput, PassParams,
        PointCloudMaterial, PointCloudMaterialPipeline, PointCloudMaterialPipelineKey,
        PointCloudMaterialPlugin, TransientTarget,
    },
    PointCloudViewSettings, PointCloudViewSettingsKey, RenderMode, StandardPasses, ViewSettings,
};

pub struct StandardPointCloudMaterialPlugin;

impl Plugin for StandardPointCloudMaterialPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        embedded_asset!(app, "pbr.wgsl");
        embedded_asset!(app, "pbr_prepass.wgsl");
        embedded_asset!(app, "pbr_normalize.wgsl");

        app.add_plugins(PointCloudMaterialPlugin::<StandardPointCloudMaterial>::default());
    }
}

#[derive(Asset, Debug, Clone, Default, Reflect, Deref, DerefMut)]
#[reflect(Default, Debug, Clone)]
pub struct StandardPointCloudMaterial {
    #[deref]
    pub material: StandardMaterial,
    pub render_mode: RenderMode,
    pub weighted_splats: bool,
}

bitflags! {
    /// The pipeline key for `StandardMaterial`, packed into 64 bits.
    #[repr(C)]
    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    pub struct StandardPointCloudMaterialExtensionKey: u64 {
        const RENDER_MODE_0            = 1_u64 <<  0;

        const RENDER_MODE_MASK         = Self::RENDER_MODE_0.bits();

        const RENDER_MODE_NORMAL       = 0;
        const RENDER_MODE_XRAY         = Self::RENDER_MODE_0.bits();

        const WEIGHTED_SPLATS_ENABLED  = 1_u64  <<  1;
    }
}

impl StandardPointCloudMaterialExtensionKey {
    pub fn render_mode(&self) -> RenderMode {
        match *self & StandardPointCloudMaterialExtensionKey::RENDER_MODE_MASK {
            StandardPointCloudMaterialExtensionKey::RENDER_MODE_NORMAL => RenderMode::Normal,
            StandardPointCloudMaterialExtensionKey::RENDER_MODE_XRAY => RenderMode::XRay,
            _ => unreachable!("Invalid render mode bits state encountered in pipeline key."),
        }
    }
    pub fn weighted_splats_enabled(&self) -> bool {
        self.contains(StandardPointCloudMaterialExtensionKey::WEIGHTED_SPLATS_ENABLED)
    }
}

impl From<&StandardPointCloudMaterial> for StandardPointCloudMaterialExtensionKey {
    fn from(material: &StandardPointCloudMaterial) -> Self {
        let mut key = StandardPointCloudMaterialExtensionKey::empty();

        key.set(
            StandardPointCloudMaterialExtensionKey::RENDER_MODE_NORMAL,
            material.render_mode == RenderMode::Normal,
        );
        key.set(
            StandardPointCloudMaterialExtensionKey::RENDER_MODE_XRAY,
            material.render_mode == RenderMode::XRay,
        );
        key.set(
            StandardPointCloudMaterialExtensionKey::WEIGHTED_SPLATS_ENABLED,
            material.weighted_splats,
        );

        key
    }
}

/// Delegate implementation of [`AsBindGroup`] with bindless disabled.
impl AsBindGroup for StandardPointCloudMaterial {
    type Data = (
        <StandardMaterial as AsBindGroup>::Data,
        StandardPointCloudMaterialExtensionKey,
    );

    type Param = <StandardMaterial as AsBindGroup>::Param;

    fn label() -> &'static str {
        "StandardPointCloudMaterial"
    }

    fn bind_group_data(&self) -> Self::Data {
        (
            <StandardMaterial as AsBindGroup>::bind_group_data(&self.material),
            self.into(),
        )
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
            &self.material,
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
        images: &RenderAssets<GpuImage>,
    ) -> StandardMaterialUniform {
        <StandardMaterial as AsBindGroupShaderType::<StandardMaterialUniform>>::as_bind_group_shader_type(&self.material, images)
    }
}

impl PointCloudMaterial for StandardPointCloudMaterial {
    type PassId = StandardPasses;

    // TODO use PointCloudViewSettings and filter points in fragment shader
    type ViewSettings = PointCloudViewSettings;

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

    fn pass_descriptor(pass: &Self::PassId) -> PassDescriptor<Self::PassId> {
        match pass {
            StandardPasses::Depth => PassDescriptor {
                id: StandardPasses::Depth,
                kind: PassKind::Geometry,
                label: "depth",
            },
            StandardPasses::Attribute => PassDescriptor {
                id: StandardPasses::Attribute,
                kind: PassKind::Geometry,
                label: "attribute",
            },
            StandardPasses::EDLNormalize => PassDescriptor {
                id: StandardPasses::EDLNormalize,
                kind: PassKind::Fullscreen,
                label: "edl_normalize",
            },
            StandardPasses::XRay => PassDescriptor {
                id: StandardPasses::XRay,
                kind: PassKind::Geometry,
                label: "xray",
            },
            StandardPasses::XRayNormalize => PassDescriptor {
                id: StandardPasses::XRayNormalize,
                kind: PassKind::Fullscreen,
                label: "xray_normalize",
            },
        }
    }

    fn prepare_pass(
        pass: &Self::PassId,
        settings_key: &<Self::ViewSettings as ViewSettings>::Key,
    ) -> Option<PassParams> {
        if matches!(pass, StandardPasses::XRay | StandardPasses::XRayNormalize)
            && !settings_key.contains(PointCloudViewSettingsKey::RENDER_MODE_XRAY)
        {
            return None;
        }
        if matches!(
            pass,
            StandardPasses::Depth | StandardPasses::Attribute | StandardPasses::EDLNormalize
        ) && !settings_key.contains(PointCloudViewSettingsKey::RENDER_MODE_MULTIPASS)
        {
            return None;
        }

        match pass {
            StandardPasses::Depth => Some(PassParams {
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
            StandardPasses::XRay | StandardPasses::Attribute => {
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
            StandardPasses::XRayNormalize | StandardPasses::EDLNormalize => Some(PassParams {
                vertex_shader: ShaderRef::Default,
                fragment_shader: shader_ref(bevy::asset::embedded_path!("pbr_normalize.wgsl")),
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
                        if matches!(pass, StandardPasses::XRayNormalize) {
                            PassInput {
                                source_pass: StandardPasses::XRay.into(),
                                binding_slot: 11,
                                texture_sample_type:
                                    bevy::render::render_resource::TextureSampleType::Float {
                                        filterable: false,
                                    },
                                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                            }
                        } else {
                            PassInput {
                                source_pass: StandardPasses::Attribute.into(),
                                binding_slot: 11,
                                texture_sample_type:
                                    bevy::render::render_resource::TextureSampleType::Float {
                                        filterable: false,
                                    },
                                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                            }
                        }
                    },
                ],
                output: PassOutput::MainColorTarget,
                blend: {
                    if matches!(pass, StandardPasses::XRayNormalize) {
                        Some(BlendState {
                            color: BlendComponent {
                                src_factor: BlendFactor::SrcAlpha,
                                dst_factor: BlendFactor::OneMinusSrcAlpha,
                                operation: BlendOperation::Add,
                            },
                            alpha: BlendComponent {
                                src_factor: BlendFactor::Zero,
                                dst_factor: BlendFactor::One,
                                operation: BlendOperation::Add,
                            },
                        })
                    } else {
                        None
                    }
                },
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
        let (material_key, point_cloud_material_key) = key.bind_group_data;

        // TODO: use this key to define a shaderdef with number of clipping planes
        let settings_key = key.view_settings_key;

        let render_mode = point_cloud_material_key.render_mode();
        let weighted_splats_enabled = point_cloud_material_key.weighted_splats_enabled();
        let edl_enabled = settings_key.edl_enabled();

        let should_render = match key.pass {
            None => {
                !matches!(render_mode, RenderMode::XRay) && !weighted_splats_enabled && !edl_enabled
            }
            Some(StandardPasses::Depth) | Some(StandardPasses::Attribute) => {
                matches!(render_mode, RenderMode::Normal)
                    && (weighted_splats_enabled || edl_enabled)
            }
            Some(StandardPasses::XRay) => {
                matches!(render_mode, RenderMode::XRay)
            }
            _ => panic!("Invalid pass in simple material specialization."),
        };

        if !should_render {
            return Ok(false);
        }

        if point_cloud_material_key
            .intersects(StandardPointCloudMaterialExtensionKey::WEIGHTED_SPLATS_ENABLED)
        {
            descriptor
                .vertex
                .shader_defs
                .push("WEIGHTED_SPLATS_ENABLED".into());
        }

        if let Some(fragment) = descriptor.fragment.as_mut() {
            let shader_defs = &mut fragment.shader_defs;

            // Evaluate single boolean flags
            for (flags, shader_def) in [
                (PointCloudViewSettingsKey::EDL_ENABLED, "RENDER_MODE_EDL"),
                (
                    PointCloudViewSettingsKey::CLIPPING_PLANES,
                    "CLIPPING_PLANES",
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
            match point_cloud_material_key.render_mode() {
                RenderMode::Normal => {
                    shader_defs.push("RENDER_MODE_NORMAL".into());
                }
                RenderMode::XRay => {
                    shader_defs.push("RENDER_MODE_XRAY".into());
                }
            }

            if point_cloud_material_key
                .intersects(StandardPointCloudMaterialExtensionKey::WEIGHTED_SPLATS_ENABLED)
            {
                shader_defs.push("WEIGHTED_SPLATS_ENABLED".into());
            }

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
                if material_key.intersects(flags) {
                    shader_defs.push(shader_def.into());
                }
            }
        }

        // Generally, we want to cull front faces if `CULL_FRONT` is present and
        // backfaces if `CULL_BACK` is present. However, if the view has
        // `INVERT_CULLING` on (usually used for mirrors and the like), we do
        // the opposite.
        descriptor.primitive.cull_mode = match (
            material_key.contains(StandardMaterialKey::CULL_FRONT),
            material_key.contains(StandardMaterialKey::CULL_BACK),
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
                (material_key.bits() >> STANDARD_MATERIAL_KEY_DEPTH_BIAS_SHIFT) as i32;
        }
        Ok(true)
    }
}

// Keep in sync with Bevy (private)
const STANDARD_MATERIAL_KEY_DEPTH_BIAS_SHIFT: u64 = 32;

impl From<Color> for StandardPointCloudMaterial {
    fn from(value: Color) -> Self {
        Self {
            material: value.into(),
            ..default()
        }
    }
}

impl From<Handle<Image>> for StandardPointCloudMaterial {
    fn from(value: Handle<Image>) -> Self {
        Self {
            material: value.into(),
            ..default()
        }
    }
}

impl From<StandardMaterial> for StandardPointCloudMaterial {
    fn from(material: StandardMaterial) -> Self {
        Self {
            material,
            ..default()
        }
    }
}
