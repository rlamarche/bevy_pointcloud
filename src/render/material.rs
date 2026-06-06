use std::marker::PhantomData;

use bevy_asset::AssetId;
use bevy_ecs::{
    prelude::World,
    query::ROQueryItem,
    resource::Resource,
    system::{
        lifetimeless::{Read, SRes},
        SystemParamItem,
    },
    world::FromWorld,
};
use bevy_render::{
    render_asset::{PrepareAssetError, RenderAsset, RenderAssets},
    render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass},
    render_resource::{BindGroupLayoutDescriptor, PipelineCache, PreparedBindGroup},
    renderer::RenderDevice,
};

use crate::point_cloud_material::{PointCloudMaterial, PointCloudMaterial3d};

/// The render world representation of a [`PointCloudMaterial`].
pub struct RenderPointCloudMaterial<M: PointCloudMaterial> {
    pub prepared_bind_group: PreparedBindGroup,
    _phantom: PhantomData<M>,
    // pub uniform_buffer: UniformBuffer<PointCloudMaterial>,
}

#[derive(Resource)]
pub struct RenderPointCloudMaterialLayout<M: PointCloudMaterial> {
    pub layout: BindGroupLayoutDescriptor,
    _phantom: PhantomData<M>,
}

impl<M: PointCloudMaterial> FromWorld for RenderPointCloudMaterialLayout<M> {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let layout = M::bind_group_layout_descriptor(render_device);
        RenderPointCloudMaterialLayout {
            layout,
            _phantom: PhantomData,
        }
    }
}

impl<M: PointCloudMaterial> RenderAsset for RenderPointCloudMaterial<M> {
    type SourceAsset = M;
    type Param = (
        SRes<RenderDevice>,
        SRes<RenderPointCloudMaterialLayout<M>>,
        SRes<PipelineCache>,
        M::Param,
    );

    fn prepare_asset(
        source_asset: Self::SourceAsset,
        _asset_id: AssetId<Self::SourceAsset>,
        (render_device, prepared_point_cloud_material_layout, pipeline_cache, param): &mut SystemParamItem<
            Self::Param,
        >,
        _: Option<&Self>,
    ) -> Result<Self, PrepareAssetError<Self::SourceAsset>> {
        let bind_group = source_asset
            .as_bind_group(
                &prepared_point_cloud_material_layout.layout,
                render_device,
                pipeline_cache,
                param,
            )
            .map_err(|err| PrepareAssetError::AsBindGroupError(err))?;

        Ok(RenderPointCloudMaterial {
            prepared_bind_group: bind_group,
            _phantom: PhantomData,
        })
    }
}

pub struct SetPointCloudMaterialGroup<const I: usize, M: PointCloudMaterial>(PhantomData<M>);

impl<const I: usize, M: PointCloudMaterial> Default for SetPointCloudMaterialGroup<I, M> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<P: PhaseItem, const I: usize, M: PointCloudMaterial> RenderCommand<P>
    for SetPointCloudMaterialGroup<I, M>
{
    type Param = SRes<RenderAssets<RenderPointCloudMaterial<M>>>;
    type ViewQuery = ();
    type ItemQuery = Read<PointCloudMaterial3d<M>>;

    fn render<'w>(
        _item: &P,
        _view: ROQueryItem<'w, '_, Self::ViewQuery>,
        point_cloud_material_3d: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        render_point_cloud_materials: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let render_point_cloud_materials = render_point_cloud_materials.into_inner();

        let Some(point_cloud_material_3d) = point_cloud_material_3d else {
            return RenderCommandResult::Skip;
        };
        let Some(render_point_cloud_material) =
            render_point_cloud_materials.get(point_cloud_material_3d)
        else {
            return RenderCommandResult::Skip;
        };

        pass.set_bind_group(
            I,
            &render_point_cloud_material.prepared_bind_group.bind_group,
            &[],
        );

        RenderCommandResult::Success
    }
}
