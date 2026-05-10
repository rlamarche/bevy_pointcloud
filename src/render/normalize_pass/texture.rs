use bevy_ecs::prelude::*;
use bevy_log::warn;
use bevy_render::{
    render_resource::{BindGroup, BindGroupEntry, IntoBinding, PipelineCache},
    renderer::RenderDevice,
    view::Msaa,
};

use crate::render::{
    attribute_pass::texture::ViewAttributePrepassTextures,
    depth_pass::texture::ViewDepthPrepassTextures, normalize_pass::pipeline::NormalizePassPipeline,
};

#[derive(Component)]
pub struct NormalizePassBindGroup {
    pub value: BindGroup,
}

pub fn prepare_normalize_pass_bind_groups(
    mut commands: Commands,
    pipeline: Res<NormalizePassPipeline>,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    views: Query<(
        Entity,
        &ViewDepthPrepassTextures,
        &ViewAttributePrepassTextures,
        &Msaa,
    )>,
) {
    for (entity, prepass_textures, attribute_textures, msaa) in &views {
        let Some(depth_texture) = &prepass_textures.depth else {
            warn!("No depth pass texture for {}", entity);
            continue;
        };
        let Some(attribute_texture) = &attribute_textures.attribute else {
            warn!("No attribute pass texture for {}", entity);
            continue;
        };

        let depth_view = depth_texture.texture.default_view.clone();
        let attribute_view = attribute_texture.texture.default_view.clone();

        let layout_descriptor = match msaa.samples() {
            1 => &pipeline.layout,
            _ => &pipeline.layout_msaa,
        };

        let layout = pipeline_cache.get_bind_group_layout(&layout_descriptor);

        commands.entity(entity).insert(NormalizePassBindGroup {
            value: render_device.create_bind_group(
                "pcl_normalize_pass_view_bind_group",
                &layout,
                &vec![
                    BindGroupEntry {
                        binding: 0,
                        resource: depth_view.into_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: attribute_view.into_binding(),
                    },
                ],
            ),
        });
    }
}
