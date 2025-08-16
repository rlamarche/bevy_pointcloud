use crate::point_cloud::{PointCloud, PointCloudData};
use bevy_asset::AssetId;
use bevy_ecs::system::SystemParamItem;
use bevy_ecs::system::lifetimeless::SRes;
use bevy_render::render_asset::{PrepareAssetError, RenderAsset};
use bevy_render::render_resource::{Buffer, BufferInitDescriptor, BufferUsages};
use bevy_render::renderer::RenderDevice;

/// The render world representation of a [`PointCloud`].
pub struct RenderPointCloud {
    pub buffer: Buffer,
    pub length: usize,
}

impl RenderAsset for RenderPointCloud {
    type SourceAsset = PointCloud;
    type Param = SRes<RenderDevice>;

    fn byte_len(source_asset: &Self::SourceAsset) -> Option<usize> {
        Some(source_asset.points.len() * size_of::<PointCloudData>())
    }

    fn prepare_asset(
        source_asset: Self::SourceAsset,
        _asset_id: AssetId<Self::SourceAsset>,
        render_device: &mut SystemParamItem<Self::Param>,
    ) -> Result<Self, PrepareAssetError<Self::SourceAsset>> {
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("PointCloud data buffer"),
            contents: bytemuck::cast_slice(source_asset.points.as_slice()),
            usage: BufferUsages::VERTEX, // | BufferUsages::COPY_DST,
        });

        Ok(RenderPointCloud {
            buffer,
            length: source_asset.points.len(),
        })
    }
}
