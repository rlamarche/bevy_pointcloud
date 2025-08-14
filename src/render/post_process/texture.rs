use crate::render::attribute_pass::texture::ViewAttributePrepassTextures;
use crate::render::depth_pass::texture::ViewDepthPrepassTextures;
use bevy_ecs::prelude::*;
use bevy_log::warn;
use bevy_render::prelude::*;
use bevy_render::render_phase::PhaseItem;
use bevy_render::render_resource::binding_types::texture_2d;
use bevy_render::render_resource::{
    BindGroup, BindGroupEntry, BindGroupLayout, IntoBinding, ShaderStages,
    TextureSampleType,
};
use bevy_render::renderer::RenderDevice;

#[derive(Resource)]
pub struct PostProcessLayout {
    pub layout: BindGroupLayout,
}

#[derive(Component)]
pub struct PostProcessBindGroup {
    pub value: BindGroup,
}

impl FromWorld for PostProcessLayout {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        PostProcessLayout {
            layout: render_device.create_bind_group_layout(
                "pcl_postprocess_layout",
                &vec![
                    texture_2d(TextureSampleType::Depth).build(0, ShaderStages::FRAGMENT),
                    texture_2d(TextureSampleType::Float { filterable: false })
                        .build(1, ShaderStages::FRAGMENT),
                ],
            ),
        }
    }
}

pub fn prepare_post_process_bind_groups(
    mut commands: Commands,
    depth_pass_layout: Res<PostProcessLayout>,
    render_device: Res<RenderDevice>,
    views: Query<(
        Entity,
        &ViewDepthPrepassTextures,
        &ViewAttributePrepassTextures,
    )>,
) {
    for (entity, prepass_textures, attribute_textures) in &views {
        let Some(depth_texture) = &prepass_textures.depth else {
            warn!("No depth texture for {}", entity);
            continue;
        };
        let Some(attribute_texture) = &attribute_textures.attribute else {
            warn!("No attribute texture for {}", entity);
            continue;
        };

        let depth_view = depth_texture.texture.default_view.clone();
        let attribute_view = attribute_texture.texture.default_view.clone();

        commands.entity(entity).insert(PostProcessBindGroup {
            value: render_device.create_bind_group(
                "pcl_prepass_view_bind_group",
                &depth_pass_layout.layout,
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
