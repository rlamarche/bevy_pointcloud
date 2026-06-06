use std::marker::PhantomData;

use bevy_asset::AssetId;
use bevy_ecs::system::{lifetimeless::SRes, SystemParamItem};
use bevy_render::{
    render_asset::{PrepareAssetError, RenderAsset},
    render_resource::{Buffer, BufferInitDescriptor, BufferUsages},
    renderer::RenderDevice,
};

use crate::{
    point::{GpuPoint, Point},
    point_cloud::PointCloud,
};

/// The render world representation of a [`PointCloud`].
pub struct RenderPointCloud<T: Point, U: GpuPoint> {
    pub buffer: Buffer,
    pub length: usize,
    pub(crate) phantom: PhantomData<fn() -> (T, U)>,
}

impl<T: Point, U: GpuPoint> RenderAsset for RenderPointCloud<T, U>
where
    for<'a> &'a T: Into<U>,
{
    type SourceAsset = PointCloud<T>;
    type Param = SRes<RenderDevice>;

    fn byte_len(source_asset: &Self::SourceAsset) -> Option<usize> {
        Some(source_asset.points.len() * size_of::<T>())
    }

    fn prepare_asset(
        source_asset: Self::SourceAsset,
        _asset_id: AssetId<Self::SourceAsset>,
        render_device: &mut SystemParamItem<Self::Param>,
        _previous_asset: Option<&Self>,
    ) -> Result<Self, PrepareAssetError<Self::SourceAsset>> {
        // convert the points into GPU representation
        let points: Vec<U> = source_asset.points.iter().map(|p| p.into()).collect();
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("PointCloud data buffer"),
            contents: bytemuck::cast_slice(points.as_slice()),
            usage: BufferUsages::VERTEX, // | BufferUsages::COPY_DST,
        });

        Ok(RenderPointCloud {
            buffer,
            length: source_asset.points.len(),
            phantom: PhantomData,
        })
    }
}
