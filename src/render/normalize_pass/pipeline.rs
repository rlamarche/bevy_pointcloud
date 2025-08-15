use crate::render::NORMALIZE_SHADER_HANDLE;
use bevy_core_pipeline::fullscreen_vertex_shader::fullscreen_shader_vertex_state;
use bevy_ecs::prelude::*;
use bevy_image::BevyDefault;
use bevy_render::{
    render_resource::{binding_types::texture_2d, *},
    renderer::RenderDevice,
};

#[derive(Resource)]
pub struct PostProcessPipeline {
    pub texture_layout: BindGroupLayout,
    pub pipeline_id: CachedRenderPipelineId,
}

impl FromWorld for PostProcessPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let textures_layout = render_device.create_bind_group_layout(
            "pcl_normalize_pass_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    // The texture containing the mask
                    // We could transmit the complete depth as f32, but we don't need
                    // Binding Depth buffer is not supported in WASM/WebGL
                    texture_2d(TextureSampleType::Uint),
                    // The texture containing the rendered point cloud (rgb = weighted sum, a = sum of weights)
                    texture_2d(TextureSampleType::Float { filterable: false }),
                ),
            ),
        );

        let pipeline_id = world
            .resource_mut::<PipelineCache>()
            // This will add the pipeline to the cache and queue its creation
            .queue_render_pipeline(RenderPipelineDescriptor {
                label: Some("post_process_pipeline".into()),
                layout: vec![textures_layout.clone()],
                // This will setup a fullscreen triangle for the vertex state
                vertex: fullscreen_shader_vertex_state(),
                fragment: Some(FragmentState {
                    shader: NORMALIZE_SHADER_HANDLE,
                    shader_defs: vec![],
                    entry_point: "fragment".into(),
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
                multisample: MultisampleState::default(),
                push_constant_ranges: vec![],
                zero_initialize_workgroup_memory: false,
            });

        Self {
            texture_layout: textures_layout,
            pipeline_id,
        }
    }
}
