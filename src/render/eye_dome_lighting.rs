use crate::render::PointCloudRenderMode;
use bevy_ecs::prelude::*;
use bevy_log::warn;
use bevy_math::{URect, Vec4};
use bevy_platform::collections::HashMap;
use bevy_platform::collections::hash_map::{Entry, VacantEntry};
use bevy_platform::hash::FixedHasher;
use bevy_render::Extract;
use bevy_render::camera::Camera;
use bevy_render::render_resource::binding_types::uniform_buffer;
use bevy_render::render_resource::{
    BindGroupLayout, BindGroupLayoutEntries, ShaderStages, ShaderType,
};
use bevy_render::renderer::RenderDevice;
use bevy_render::sync_world::RenderEntity;
use std::marker::PhantomData;

#[derive(Resource, Default)]
pub struct NeighboursCache<'w> {
    mesh_layout_cache: HashMap<u32, [Vec4; 4]>,
    _marker: PhantomData<&'w ()>,
}

impl<'w> NeighboursCache<'w> {
    #[inline]
    pub fn get_neighbours(&mut self, neighbours_count: u32) -> &[Vec4; 4] {
        return match self.mesh_layout_cache.entry(neighbours_count) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => get_neighbours_slow(neighbours_count, entry),
        };

        #[cold]
        fn get_neighbours_slow(
            neighbours_count: u32,
            entry: VacantEntry<u32, [Vec4; 4], FixedHasher>,
        ) -> &[Vec4; 4] {
            let neighbours: [Vec4; 4] = std::array::from_fn(|i| {
                // j in (0..4) for neighbours_count = 4
                // j in (4..8) for neighbours_count = 8
                let j = (2 * i) % neighbours_count as usize;
                let angle_0 = 2.0 * i as f32 * std::f32::consts::PI / neighbours_count as f32;
                let angle_1 = 2.0 * j as f32 * std::f32::consts::PI / neighbours_count as f32;
                Vec4::new(angle_0.cos(), angle_0.sin(), angle_1.cos(), angle_1.sin())
            });

            entry.insert(neighbours)
        }
    }
}

#[derive(Component, ShaderType, Clone, Copy)]
pub struct EyeDomeLightingUniform {
    pub strength: f32,
    pub radius: f32,
    pub screen_width: f32,
    pub screen_height: f32,
    pub neighbours: [Vec4; 4],
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
            &BindGroupLayoutEntries::single(
                ShaderStages::FRAGMENT,
                uniform_buffer::<EyeDomeLightingUniform>(false),
            ),
        );
        Self { layout }
    }
}

pub fn extract_cameras_render_mode(
    mut commands: Commands,
    query: Extract<Query<(Entity, &Camera, &PointCloudRenderMode)>>,
    mut neighbours_cache: ResMut<NeighboursCache<'static>>,
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
                        neighbours: neighbours_cache
                            .get_neighbours(render_mode.edl_neighbour_count)
                            .clone(),
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
