use crate::render::PointCloudRenderMode;
use bevy_ecs::prelude::*;
use bevy_log::warn;
use bevy_math::URect;
use bevy_render::Extract;
use bevy_render::camera::Camera;
use bevy_render::render_resource::binding_types::{
    storage_buffer_read_only_sized, uniform_buffer,
};
use bevy_render::render_resource::{
    BindGroupLayout, BindGroupLayoutEntries, ShaderStages, ShaderType,
};
use bevy_render::renderer::RenderDevice;
use bevy_render::sync_world::RenderEntity;

#[derive(Component, ShaderType, Clone, Copy)]
pub struct EyeDomeLightingUniform {
    pub strength: f32,
    pub radius: f32,
    pub screen_width: f32,
    pub screen_height: f32,
}

#[derive(Resource)]
pub struct EyeDomeLightingUniformBindgroupLayout {
    pub layout: BindGroupLayout,
}

impl FromWorld for EyeDomeLightingUniformBindgroupLayout {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = render_device.create_bind_group_layout(
            "EyeDomeLighting layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    uniform_buffer::<EyeDomeLightingUniform>(false),
                    storage_buffer_read_only_sized(false, None),
                ),
            ),
        );

        Self { layout }
    }
}

pub fn extract_cameras_render_mode(
    mut commands: Commands,
    query: Extract<Query<(Entity, &Camera, &PointCloudRenderMode)>>,
    mapper: Extract<Query<&RenderEntity>>,
) {
    for (main_entity, camera, render_mode) in query.iter() {
        let result = mapper.get(main_entity);

        let (
            Some(URect {
                min: viewport_origin,
                ..
            }),
            Some(viewport_size),
            Some(target_size),
        ) = (
            camera.physical_viewport_rect(),
            camera.physical_viewport_size(),
            camera.physical_target_size(),
        )
        else {
            continue;
        };

        match result {
            Ok(render_entity) => {
                commands.entity(**render_entity).insert((
                    EyeDomeLightingUniform {
                        strength: render_mode.edl_strength,
                        radius: render_mode.edl_radius,
                        screen_width: target_size.x as f32,
                        screen_height: target_size.y as f32,
                    },
                    // we also need the render mode information to have the neighbours count
                    render_mode.clone(),
                ));
            }
            Err(error) => {
                warn!("Corresponding extracted view not found.");
            }
        }
    }
}
