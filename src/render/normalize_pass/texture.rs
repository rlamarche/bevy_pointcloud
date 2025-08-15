use crate::render::attribute_pass::texture::ViewAttributePrepassTextures;
use crate::render::depth_pass::texture::ViewDepthPrepassTextures;
use crate::render::normalize_pass::pipeline::NormalizePassPipeline;
use bevy_ecs::prelude::*;
use bevy_log::warn;
use bevy_render::render_resource::{BindGroup, BindGroupEntry, IntoBinding};
use bevy_render::renderer::RenderDevice;
use bevy_render::view::Msaa;

#[derive(Component)]
pub struct NormalizePassBindGroup {
    pub value: BindGroup,
}

pub fn prepare_normalize_pass_bind_groups(
    mut commands: Commands,
    pipeline: Res<NormalizePassPipeline>,
    render_device: Res<RenderDevice>,
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

        let layout = match msaa.samples() {
            1 => vec![&pipeline.layout],
            _ => vec![&pipeline.layout_msaa],
        };

        commands.entity(entity).insert(NormalizePassBindGroup {
            value: render_device.create_bind_group(
                "pcl_normalize_pass_view_bind_group",
                layout[0],
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
