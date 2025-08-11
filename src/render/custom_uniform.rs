use crate::render::CustomPipeline;
use bevy_ecs::prelude::*;
use bevy_ecs::query::ROQueryItem;
use bevy_ecs::system::SystemParamItem;
use bevy_ecs::system::lifetimeless::Read;
use bevy_math::Mat4;
use bevy_reflect::TypePath;
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass};
use bevy_render::render_resource::{AsBindGroup, PreparedBindGroup};
use bevy_render::renderer::RenderDevice;

#[derive(Component, TypePath, AsBindGroup, Clone, Debug)]
pub struct CustomUniform {
    #[uniform(0)]
    pub world_from_local: Mat4,
}

#[derive(Component)]
pub struct PreparedCustomUniform {
    prepared: PreparedBindGroup<()>,
}

pub fn prepare_custom_uniform<'w>(
    mut commands: Commands,
    custom_pipeline: Res<CustomPipeline>,
    query: Query<(Entity, &CustomUniform)>,
    render_device: Res<RenderDevice>,
    mut material: (
        Res<'w, RenderAssets<bevy_render::texture::GpuImage>>,
        Res<'w, bevy_render::texture::FallbackImage>,
        Res<'w, RenderAssets<bevy_render::storage::GpuShaderStorageBuffer>>,
    ),
) {
    let render_device = render_device.into_inner();

    for (entity, custom_uniform) in &query {
        let prepared = custom_uniform
            .as_bind_group(&custom_pipeline.custom_layout, render_device, &mut material)
            .expect("Unable to build bind group from CustomUniform.");

        commands
            .entity(entity)
            .insert(PreparedCustomUniform { prepared });
    }
}

pub struct SetCustomUniformGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetCustomUniformGroup<I> {
    type Param = ();
    type ViewQuery = ();
    type ItemQuery = Read<PreparedCustomUniform>;

    fn render<'w>(
        _item: &P,
        _view: ROQueryItem<'w, Self::ViewQuery>,
        prepared_custom_uniform: Option<ROQueryItem<'w, Self::ItemQuery>>,
        _param: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(prepared_custom_uniform) = prepared_custom_uniform else {
            return RenderCommandResult::Skip;
        };

        pass.set_bind_group(I, &prepared_custom_uniform.prepared.bind_group, &[]);

        RenderCommandResult::Success
    }
}
