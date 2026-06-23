use bevy_ecs::{
    query::ROQueryItem,
    system::{
        lifetimeless::{Read, SRes},
        SystemParamItem,
    },
};
use bevy_pbr::MaterialBindGroupAllocators;
use bevy_render::render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass};

use crate::{
    point_cloud_material::{
        PointCloudMaterialKey, PreparedPointCloudMaterial, RenderPointCloudMaterialInstances,
    },
    render_asset::{ErasedRenderAssetsComponent, RenderAssetKey},
};

// /// The render world representation of a [`PointCloudMaterial`].
// pub struct RenderPointCloudMaterial<M: PointCloudMaterial> {
//     pub prepared_bind_group: PreparedBindGroup,
//     _phantom: PhantomData<M>,
//     // pub uniform_buffer: UniformBuffer<PointCloudMaterial>,
// }

// #[derive(Resource)]
// pub struct RenderPointCloudMaterialLayout<M: PointCloudMaterial> {
//     pub layout: BindGroupLayoutDescriptor,
//     _phantom: PhantomData<M>,
// }

// impl<M: PointCloudMaterial> FromWorld for RenderPointCloudMaterialLayout<M> {
//     fn from_world(world: &mut World) -> Self {
//         let render_device = world.resource::<RenderDevice>();
//         let layout = M::bind_group_layout_descriptor(render_device);
//         RenderPointCloudMaterialLayout {
//             layout,
//             _phantom: PhantomData,
//         }
//     }
// }

// impl<M: PointCloudMaterial> RenderAsset for RenderPointCloudMaterial<M> {
//     type SourceAsset = M;
//     type Param = (
//         SRes<RenderDevice>,
//         SRes<RenderPointCloudMaterialLayout<M>>,
//         SRes<PipelineCache>,
//         M::Param,
//     );

//     fn prepare_asset(
//         source_asset: Self::SourceAsset,
//         _asset_id: AssetId<Self::SourceAsset>,
//         (render_device, prepared_point_cloud_material_layout, pipeline_cache, param): &mut SystemParamItem<
//             Self::Param,
//         >,
//         _: Option<&Self>,
//     ) -> Result<Self, PrepareAssetError<Self::SourceAsset>> {
//         let bind_group = source_asset
//             .as_bind_group(
//                 &prepared_point_cloud_material_layout.layout,
//                 render_device,
//                 pipeline_cache,
//                 param,
//             )
//             .map_err(|err| PrepareAssetError::AsBindGroupError(err))?;

//         Ok(RenderPointCloudMaterial {
//             prepared_bind_group: bind_group,
//             _phantom: PhantomData,
//         })
//     }
// }

pub struct SetPointCloudMaterialGroup<const I: usize>;

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetPointCloudMaterialGroup<I> {
    type Param = (
        SRes<ErasedRenderAssetsComponent<PreparedPointCloudMaterial, PointCloudMaterialKey>>,
        SRes<RenderPointCloudMaterialInstances>,
        SRes<MaterialBindGroupAllocators>,
    );
    type ViewQuery = ();
    type ItemQuery = Read<RenderAssetKey<PointCloudMaterialKey>>;

    fn render<'w>(
        item: &P,
        _view: (),
        render_material_key: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        (materials, material_instances, material_bind_group_allocator): SystemParamItem<
            'w,
            '_,
            Self::Param,
        >,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let materials = materials.into_inner();
        let material_instances = material_instances.into_inner();
        let material_bind_group_allocators = material_bind_group_allocator.into_inner();

        let Some(material_instance) = material_instances.instances.get(&item.main_entity()) else {
            return RenderCommandResult::Skip;
        };
        let Some(material_bind_group_allocator) =
            material_bind_group_allocators.get(&material_instance.asset_id.type_id())
        else {
            return RenderCommandResult::Skip;
        };
        let Some(render_material_key) = render_material_key else {
            return RenderCommandResult::Skip;
        };
        let Some(material) =
            materials.get((material_instance.asset_id, render_material_key.clone()))
        else {
            return RenderCommandResult::Skip;
        };
        let Some(material_bind_group) = material_bind_group_allocator.get(material.binding.group)
        else {
            return RenderCommandResult::Skip;
        };
        let Some(bind_group) = material_bind_group.bind_group() else {
            return RenderCommandResult::Skip;
        };
        pass.set_bind_group(I, bind_group, &[]);
        RenderCommandResult::Success
    }
}
