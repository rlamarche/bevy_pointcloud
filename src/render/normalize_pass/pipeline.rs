use bevy_core_pipeline::FullscreenShader;
use bevy_ecs::prelude::*;
use bevy_image::BevyDefault;
use bevy_render::render_resource::{
    binding_types::{texture_2d, texture_2d_multisampled},
    *,
};
use bevy_shader::ShaderDefVal;

use crate::render::{
    normalize_pass::EyeDomeLightingUniformBindgroupLayout, NORMALIZE_SHADER_HANDLE,
};

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct NormalizePassPipelineKey {
    pub samples: u32,
    pub use_edl: bool,
    pub edl_neighbour_count: u32,
}

#[derive(Component)]
pub struct NormalizePassPipelineId(pub CachedRenderPipelineId);

#[derive(Resource)]
pub struct NormalizePassPipeline {
    pub layout: BindGroupLayoutDescriptor,
    pub layout_msaa: BindGroupLayoutDescriptor,
    pub edl_layout: BindGroupLayoutDescriptor,
    fullscreen_shader: FullscreenShader,
}

impl FromWorld for NormalizePassPipeline {
    fn from_world(world: &mut World) -> Self {
        let layout = BindGroupLayoutDescriptor {
            label: "pcl_normalize_pass_bind_group_layout".into(),
            entries: BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    // The texture containing the depth
                    // Binding Depth buffer is not supported in WASM/WebGL
                    texture_2d(TextureSampleType::Float { filterable: false }),
                    // The texture containing the rendered point cloud (rgb = weighted sum, a = sum of weights)
                    texture_2d(TextureSampleType::Float { filterable: false }),
                ),
            )
            .to_vec(),
        };
        let layout_msaa = BindGroupLayoutDescriptor {
            label: "pcl_normalize_pass_bind_group_layout_msaa".into(),
            entries: BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    // The texture containing the mask
                    // We could transmit the complete depth as f32, but we don't need
                    // Binding Depth buffer is not supported in WASM/WebGL
                    texture_2d_multisampled(TextureSampleType::Float { filterable: false }),
                    // The texture containing the rendered point cloud (rgb = weighted sum, a = sum of weights)
                    texture_2d_multisampled(TextureSampleType::Float { filterable: false }),
                ),
            )
            .to_vec(),
        };
        let edl_layout = world
            .resource::<EyeDomeLightingUniformBindgroupLayout>()
            .layout
            .clone();

        Self {
            layout,
            layout_msaa,
            edl_layout,
            fullscreen_shader: world.resource::<FullscreenShader>().clone(),
        }
    }
}

impl SpecializedRenderPipeline for NormalizePassPipeline {
    type Key = NormalizePassPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let mut layout = match key.samples {
            1 => vec![self.layout.clone()],
            _ => vec![self.layout_msaa.clone()],
        };

        let mut shader_defs = Vec::new();
        if key.samples > 1 {
            shader_defs.push("MULTISAMPLED".into());
        }
        if key.use_edl {
            layout.push(self.edl_layout.clone());
            shader_defs.push("USE_EDL".into());
            shader_defs.push(ShaderDefVal::UInt(
                "NEIGHBOUR_COUNT".into(),
                key.edl_neighbour_count,
            ));
        }

        RenderPipelineDescriptor {
            label: Some("pcl_normalize_pass_pipeline".into()),
            layout,
            // This will setup a fullscreen triangle for the vertex state
            vertex: self.fullscreen_shader.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: NORMALIZE_SHADER_HANDLE,
                shader_defs,
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format: TextureFormat::bevy_default(),
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                // Do not write the depth, because it has already been written in the depth pass, and we don't have access to it
                depth_write_enabled: false,
                depth_compare: CompareFunction::Always,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: MultisampleState {
                count: key.samples,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            push_constant_ranges: vec![],
            zero_initialize_workgroup_memory: false,
        }
    }
}
