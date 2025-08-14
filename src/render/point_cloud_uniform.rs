use bevy_ecs::prelude::*;
use bevy_ecs::query::ROQueryItem;
use bevy_ecs::system::lifetimeless::Read;
use bevy_ecs::system::SystemParamItem;
use bevy_math::Mat4;
use bevy_reflect::TypePath;
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass};
use bevy_render::render_resource::{AsBindGroup, BindGroupLayout, PreparedBindGroup};
use bevy_render::renderer::RenderDevice;

#[derive(Component, TypePath, AsBindGroup, Clone, Debug)]
pub struct PointCloudUniform {
    #[uniform(0)]
    pub world_from_local: Mat4,
}

#[derive(Component)]
pub struct PreparedPointCloudUniform {
    prepared: PreparedBindGroup<()>,
}
#[derive(Resource)]
pub struct PointCloudUniformLayout {
    pub layout: BindGroupLayout,
}

impl FromWorld for PointCloudUniformLayout {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        PointCloudUniformLayout {
            layout: PointCloudUniform::bind_group_layout(render_device),
        }
    }
}

pub fn prepare_point_cloud_uniform<'w>(
    mut commands: Commands,
    point_cloud_uniform_layout: Res<PointCloudUniformLayout>,
    query: Query<(Entity, &PointCloudUniform)>,
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
            .as_bind_group(&point_cloud_uniform_layout.layout, render_device, &mut material)
            .expect("Unable to build bind group from PointCloudUniform.");

        commands
            .entity(entity)
            .insert(PreparedPointCloudUniform { prepared });
    }
}

pub struct SetPointCloudUniformGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetPointCloudUniformGroup<I> {
    type Param = ();
    type ViewQuery = ();
    type ItemQuery = Read<PreparedPointCloudUniform>;

    fn render<'w>(
        _item: &P,
        _view: ROQueryItem<'w, Self::ViewQuery>,
        prepared_custom_uniform: Option<ROQueryItem<'w, Self::ItemQuery>>,
        _param: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(prepared_point_cloud_uniform) = prepared_custom_uniform else {
            return RenderCommandResult::Skip;
        };

        pass.set_bind_group(I, &prepared_point_cloud_uniform.prepared.bind_group, &[]);

        RenderCommandResult::Success
    }
}
