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
    render_resource::{
        binding_types::uniform_buffer, BindGroup, BindGroupEntries, BindGroupLayout,
        BindGroupLayoutEntries, ShaderStages, UniformBuffer,
    },
    renderer::{RenderDevice, RenderQueue},
};

use crate::point_cloud_material::{PointCloudMaterial, PointCloudMaterial3d};

/// The render world representation of a [`PointCloudMaterial`].
pub struct RenderPointCloudMaterial {
    pub uniform: BindGroup,
    pub uniform_buffer: UniformBuffer<PointCloudMaterial>,
}

#[derive(Resource)]
pub struct RenderPointCloudMaterialLayout {
    pub layout: BindGroupLayout,
}

impl FromWorld for RenderPointCloudMaterialLayout {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let layout = render_device.create_bind_group_layout(
            "pcl_material",
            &BindGroupLayoutEntries::single(
                ShaderStages::VERTEX,
                uniform_buffer::<PointCloudMaterial>(false),
            ),
        );
        RenderPointCloudMaterialLayout { layout }
    }
}

impl RenderAsset for RenderPointCloudMaterial {
    type SourceAsset = PointCloudMaterial;
    type Param = (
        SRes<RenderDevice>,
        SRes<RenderQueue>,
        SRes<RenderPointCloudMaterialLayout>,
    );

    fn prepare_asset(
        source_asset: Self::SourceAsset,
        _asset_id: AssetId<Self::SourceAsset>,
        (render_device, render_queue, prepared_point_cloud_material_layout): &mut SystemParamItem<
            Self::Param,
        >,
        _: Option<&Self>,
    ) -> Result<Self, PrepareAssetError<Self::SourceAsset>> {
        let mut uniform_buffer = UniformBuffer::from(source_asset);
        uniform_buffer.write_buffer(render_device, render_queue);

        let uniform = render_device.create_bind_group(
            "pcl_material",
            &prepared_point_cloud_material_layout.layout,
            &BindGroupEntries::single(uniform_buffer.binding().unwrap()),
        );

        Ok(RenderPointCloudMaterial {
            uniform,
            uniform_buffer,
        })
    }
}

pub struct SetPointCloudMaterialGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetPointCloudMaterialGroup<I> {
    type Param = SRes<RenderAssets<RenderPointCloudMaterial>>;
    type ViewQuery = ();
    type ItemQuery = Read<PointCloudMaterial3d>;

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

        pass.set_bind_group(I, &render_point_cloud_material.uniform, &[]);

        RenderCommandResult::Success
    }
}
