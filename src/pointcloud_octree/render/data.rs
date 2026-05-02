use bevy_ecs::prelude::*;
use bevy_ecs::query::ROQueryItem;
use bevy_ecs::system::lifetimeless::Read;
use bevy_ecs::system::SystemParamItem;
use bevy_math::prelude::*;
use bevy_reflect::TypePath;
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass};
use bevy_render::render_resource::{AsBindGroup, BindGroupLayoutDescriptor, PipelineCache, PreparedBindGroup};
use bevy_render::renderer::RenderDevice;

#[derive(Component, TypePath, AsBindGroup, Clone, Debug)]
pub struct PointCloudOctree3dUniform {
    #[uniform(0)]
    pub world_from_local: Mat4,
}

#[derive(Component)]
pub struct PreparedPointCloudOctree3dUniform {
    pub prepared: PreparedBindGroup,
}
#[derive(Resource)]
pub struct PointCloudOctree3dUniformLayout {
    pub layout: BindGroupLayoutDescriptor,
}

impl FromWorld for PointCloudOctree3dUniformLayout {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        Self {
            layout: PointCloudOctree3dUniform::bind_group_layout_descriptor(render_device),
        }
    }
}

pub fn prepare_point_cloud_octree_3d_uniform<'w>(
    mut commands: Commands,
    point_cloud_octree_3d_uniform_layout: Res<PointCloudOctree3dUniformLayout>,
    query: Query<(Entity, &PointCloudOctree3dUniform)>,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    mut material: (
        Res<'w, RenderAssets<bevy_render::texture::GpuImage>>,
        Res<'w, bevy_render::texture::FallbackImage>,
        Res<'w, RenderAssets<bevy_render::storage::GpuShaderStorageBuffer>>,
    ),
) {
    let render_device = render_device.into_inner();
    let pipeline_cache = pipeline_cache.into_inner();

    for (entity, custom_uniform) in &query {
        let prepared = custom_uniform
            .as_bind_group(
                &point_cloud_octree_3d_uniform_layout.layout,
                render_device,
                pipeline_cache,
                &mut material,
            )
            .expect("Unable to build bind group from PointCloudUniform.");

        commands
            .entity(entity)
            .insert(PreparedPointCloudOctree3dUniform { prepared });
    }
}

pub struct SetPointCloudOctree3dUniformGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetPointCloudOctree3dUniformGroup<I> {
    type Param = ();
    type ViewQuery = ();
    type ItemQuery = Read<PreparedPointCloudOctree3dUniform>;

    fn render<'w>(
        _item: &P,
        _view: ROQueryItem<'w, '_, Self::ViewQuery>,
        prepared_point_cloud_octree_3d_uniform: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        _param: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(prepared_point_cloud_uniform) = prepared_point_cloud_octree_3d_uniform else {
            return RenderCommandResult::Skip;
        };

        pass.set_bind_group(I, &prepared_point_cloud_uniform.prepared.bind_group, &[]);

        RenderCommandResult::Success
    }
}
