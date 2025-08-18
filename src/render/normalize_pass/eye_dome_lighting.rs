use crate::render::PointCloudRenderMode;
use crate::render::eye_dome_lighting::{
    EyeDomeLightingUniform, EyeDomeLightingUniformBindgroupLayout,
};
use bevy_ecs::prelude::*;
use bevy_render::extract_component::{ComponentUniforms, DynamicUniformIndex};
use bevy_render::render_resource::{
    BindGroup, BindGroupEntries,
};
use bevy_render::renderer::RenderDevice;
use bevy_render::view::Msaa;

#[derive(Component)]
pub struct NormalizePassEdlBindgroup {
    pub value: BindGroup,
}

pub fn prepare_normalize_pass_edl_bind_groups(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    edl_layout: Res<EyeDomeLightingUniformBindgroupLayout>,
    edl_uniforms: Res<ComponentUniforms<EyeDomeLightingUniform>>,
    views: Query<(
        Entity,
        &Msaa,
        &PointCloudRenderMode,
        &DynamicUniformIndex<EyeDomeLightingUniform>,
    )>,
) {
    for (entity, msaa, point_cloud_render_mode, edl_index) in &views {
        if let Some(edl_uniforms_binding) = edl_uniforms.uniforms().binding() {
            let value = render_device.create_bind_group(
                "pcl_normalize_pass_edl_bind_group",
                &edl_layout.layout,
                &BindGroupEntries::single(
                    edl_uniforms_binding,
                ),
            );

            commands
                .entity(entity)
                .insert(NormalizePassEdlBindgroup { value });
        }
    }
}
