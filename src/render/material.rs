use crate::point_cloud_material::{PointCloudMaterial, PointCloudMaterial3d};
use bevy_asset::AssetId;
use bevy_ecs::prelude::World;
use bevy_ecs::query::ROQueryItem;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::lifetimeless::{Read, SRes};
use bevy_ecs::system::SystemParamItem;
use bevy_ecs::world::FromWorld;
use bevy_render::render_asset::{PrepareAssetError, RenderAsset, RenderAssets};
use bevy_render::render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass};
use bevy_render::render_resource::{AsBindGroup, BindGroupLayout, PreparedBindGroup};
use bevy_render::renderer::RenderDevice;

/// The render world representation of a [`PointCloudMaterial`].
pub struct RenderPointCloudMaterial {
    pub prepared: PreparedBindGroup<()>,
}

#[derive(Resource)]
pub struct RenderPointCloudMaterialLayout {
    pub layout: BindGroupLayout,
}

impl FromWorld for RenderPointCloudMaterialLayout {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let layout = PointCloudMaterial::bind_group_layout(&render_device);
        RenderPointCloudMaterialLayout { layout }
    }
}

impl RenderAsset for RenderPointCloudMaterial {
    type SourceAsset = PointCloudMaterial;
    type Param = (
        SRes<RenderDevice>,
        SRes<RenderPointCloudMaterialLayout>,
        (
            SRes<RenderAssets<bevy_render::texture::GpuImage>>,
            SRes<bevy_render::texture::FallbackImage>,
            SRes<RenderAssets<bevy_render::storage::GpuShaderStorageBuffer>>,
        ),
    );

    fn prepare_asset(
        source_asset: Self::SourceAsset,
        _asset_id: AssetId<Self::SourceAsset>,
        (render_device, prepared_point_cloud_material_layout, material): &mut SystemParamItem<
            Self::Param,
        >,
    ) -> Result<Self, PrepareAssetError<Self::SourceAsset>> {
        match source_asset.as_bind_group(
            &prepared_point_cloud_material_layout.layout,
            render_device,
            material,
        ) {
            Ok(prepared) => Ok(RenderPointCloudMaterial { prepared }),
            Err(error) => Err(PrepareAssetError::AsBindGroupError(error)),
        }
    }
}

pub struct SetPointCloudMaterialGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetPointCloudMaterialGroup<I> {
    type Param = SRes<RenderAssets<RenderPointCloudMaterial>>;
    type ViewQuery = ();
    type ItemQuery = Read<PointCloudMaterial3d>;

    fn render<'w>(
        _item: &P,
        _view: ROQueryItem<'w, Self::ViewQuery>,
        point_cloud_material_3d: Option<ROQueryItem<'w, Self::ItemQuery>>,
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

        pass.set_bind_group(I, &render_point_cloud_material.prepared.bind_group, &[]);

        RenderCommandResult::Success
    }
}
