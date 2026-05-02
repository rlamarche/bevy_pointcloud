use crate::render::PointCloudRenderMode;
use crate::render::eye_dome_lighting::{
    EyeDomeLightingUniform, EyeDomeLightingUniformBindgroupLayout,
};
use bevy_ecs::prelude::*;
use bevy_render::extract_component::{ComponentUniforms, DynamicUniformIndex};
use bevy_render::render_resource::{BindGroup, BindGroupEntries, PipelineCache};
use bevy_render::renderer::RenderDevice;
use bevy_render::view::Msaa;

#[derive(Component)]
pub struct NormalizePassEdlBindgroup {
    pub value: BindGroup,
}

pub fn prepare_normalize_pass_edl_bind_groups(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    edl_layout: Res<EyeDomeLightingUniformBindgroupLayout>,
    edl_uniforms: Res<ComponentUniforms<EyeDomeLightingUniform>>,
    views: Query<(
        Entity,
        &Msaa,
        &PointCloudRenderMode,
        &DynamicUniformIndex<EyeDomeLightingUniform>,
    )>,
) {
    for (entity, _msaa, _point_cloud_render_mode, _edl_index) in &views {
        if let Some(edl_uniforms_binding) = edl_uniforms.uniforms().binding() {
            let value = render_device.create_bind_group(
                "pcl_normalize_pass_edl_bind_group",
                &pipeline_cache.get_bind_group_layout(&edl_layout.layout),
                &BindGroupEntries::single(edl_uniforms_binding),
            );

            commands
                .entity(entity)
                .insert(NormalizePassEdlBindgroup { value });
        }
    }
}
