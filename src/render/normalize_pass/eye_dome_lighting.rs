use crate::render::PointCloudRenderMode;
use crate::render::eye_dome_lighting::{
    EyeDomeLightingUniform, EyeDomeLightingUniformBindgroupLayout,
};
use bevy_ecs::prelude::*;
use bevy_math::Vec2;
use bevy_platform::collections::HashMap;
use bevy_platform::collections::hash_map::{Entry, VacantEntry};
use bevy_platform::hash::FixedHasher;
use bevy_render::extract_component::{ComponentUniforms, DynamicUniformIndex};
use bevy_render::render_resource::{
    BindGroup, BindGroupEntries, Buffer, BufferInitDescriptor, BufferUsages,
};
use bevy_render::renderer::RenderDevice;
use bevy_render::view::Msaa;
use std::marker::PhantomData;

#[derive(Resource, Default)]
pub struct NeighboursCache<'w> {
    mesh_layout_cache: HashMap<u32, Buffer>,
    _marker: PhantomData<&'w ()>,
}

impl<'w> NeighboursCache<'w> {
    pub fn get_neighbours(
        &mut self,
        render_device: &RenderDevice,
        neighbours_count: u32,
    ) -> &Buffer {
        return match self.mesh_layout_cache.entry(neighbours_count) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => get_neighbours_slow(render_device, neighbours_count, entry),
        };

        fn get_neighbours_slow<'a>(
            render_device: &RenderDevice,
            neighbours_count: u32,
            entry: VacantEntry<'a, u32, Buffer, FixedHasher>,
        ) -> &'a Buffer {
            let mut neighbours = Vec::with_capacity(neighbours_count as usize);

            for i in 0..neighbours_count {
                let angle = 2.0 * i as f32 * std::f32::consts::PI / neighbours_count as f32;
                neighbours.push(Vec2::new(angle.cos(), angle.sin()));
            }

            let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("PointCloud data buffer"),
                contents: bytemuck::cast_slice(neighbours.as_slice()),
                usage: BufferUsages::STORAGE, // | BufferUsages::COPY_DST,
            });
            entry.insert(buffer)
        }
    }
}

#[derive(Component)]
pub struct NormalizePassEdlBindgroup {
    pub value: BindGroup,
}

pub fn prepare_normalize_pass_edl_bind_groups(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    edl_layout: Res<EyeDomeLightingUniformBindgroupLayout>,
    edl_uniforms: Res<ComponentUniforms<EyeDomeLightingUniform>>,
    mut neighbours_cache: ResMut<NeighboursCache<'static>>,
    views: Query<(
        Entity,
        &Msaa,
        &PointCloudRenderMode,
        &DynamicUniformIndex<EyeDomeLightingUniform>,
    )>,
) {
    for (entity, msaa, point_cloud_render_mode, edl_index) in &views {
        // TODO create a cache with this
        // let mut neighbours =
        //     Vec::with_capacity(point_cloud_render_mode.edl_neighbour_count as usize);
        //
        // for i in 0..point_cloud_render_mode.edl_neighbour_count {
        //     let angle = 2.0 * i as f32 * std::f32::consts::PI
        //         / point_cloud_render_mode.edl_neighbour_count as f32;
        //     neighbours.push(Vec2::new(angle.cos(), angle.sin()));
        // }
        //
        // let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        //     label: Some("PointCloud data buffer"),
        //     contents: bytemuck::cast_slice(neighbours.as_slice()),
        //     usage: BufferUsages::STORAGE, // | BufferUsages::COPY_DST,
        // });

        if let Some(edl_uniforms_binding) = edl_uniforms.uniforms().binding() {
            let neighbours = neighbours_cache
                .get_neighbours(&render_device, point_cloud_render_mode.edl_neighbour_count);

            let value = render_device.create_bind_group(
                "pcl_normalize_pass_edl_bind_group",
                &edl_layout.layout,
                &BindGroupEntries::sequential((
                    edl_uniforms_binding,
                    neighbours.as_entire_binding(),
                )),
            );

            commands
                .entity(entity)
                .insert(NormalizePassEdlBindgroup { value });
        }
    }
}
