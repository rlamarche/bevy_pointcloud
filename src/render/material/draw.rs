use bevy::{
    ecs::system::{lifetimeless::SRes, SystemParamItem},
    log::{info, warn},
    pbr::{MaterialBindGroupAllocators, SetMeshViewBindGroup, SetMeshViewBindingArrayBindGroup},
    render::{
        erased_render_asset::ErasedRenderAssets,
        render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass},
    },
};

use crate::render::{
    DrawPointCloudInstanced, MySetItemPipeline, PreparedPointCloudMaterial,
    RenderPointCloudChunkInstances, RenderPointCloudMaterialInstances, SetPointCloudBindGroup,
};

pub type DrawPointCloudMaterial = (
    MySetItemPipeline,
    // SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshViewBindingArrayBindGroup<1>,
    SetPointCloudBindGroup<2>,
    SetMaterialBindGroup<3>,
    DrawPointCloudInstanced,
);

/// Sets the bind group for a given [`Material`] at the configured `I` index.
pub struct SetMaterialBindGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetMaterialBindGroup<I> {
    type Param = (
        SRes<RenderPointCloudChunkInstances>,
        SRes<ErasedRenderAssets<PreparedPointCloudMaterial>>,
        SRes<RenderPointCloudMaterialInstances>,
        SRes<MaterialBindGroupAllocators>,
    );
    type ViewQuery = ();
    type ItemQuery = ();

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        _item_query: Option<()>,
        (
            render_point_cloud_chunk_instances,
            materials,
            material_instances,
            material_bind_group_allocator,
        ): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let materials = materials.into_inner();
        let material_instances = material_instances.into_inner();
        let material_bind_group_allocators = material_bind_group_allocator.into_inner();

        let Some(chunk_instance) = render_point_cloud_chunk_instances.get(&item.entity()) else {
            warn!("render_point_cloud_chunk_instance missing 4");
            return RenderCommandResult::Skip;
        };

        let Some(material_instance) = material_instances
            .instances
            .get(&chunk_instance.root_entity)
        else {
            info!("missing material 1");
            return RenderCommandResult::Skip;
        };
        let Some(material_bind_group_allocator) =
            material_bind_group_allocators.get(&material_instance.asset_id.type_id())
        else {
            info!("missing material 2");
            return RenderCommandResult::Skip;
        };
        let Some(material) = materials.get(material_instance.asset_id) else {
            info!("missing material 3");

            return RenderCommandResult::Skip;
        };
        // info!("Material: {:?}", material.binding);
        let Some(material_bind_group) = material_bind_group_allocator.get(material.binding.group)
        else {
            info!("missing material 4");

            return RenderCommandResult::Skip;
        };
        let Some(bind_group) = material_bind_group.bind_group() else {
            info!("missing material 5");

            return RenderCommandResult::Skip;
        };
        pass.set_bind_group(I, bind_group, &[]);

        RenderCommandResult::Success
    }
}
