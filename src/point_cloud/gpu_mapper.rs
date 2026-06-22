use std::{any::TypeId, hash::Hash, marker::PhantomData, sync::Arc};

use bevy_app::prelude::*;
use bevy_asset::{prelude::*, RenderAssetUsages};
use bevy_ecs::{
    prelude::*,
    system::{lifetimeless::SRes, SystemParam, SystemParamItem},
};
use bevy_mesh::VertexBufferLayout;
use bevy_reflect::TypePath;
use bevy_render::{
    erased_render_asset::{
        ErasedRenderAsset, ErasedRenderAssetDependency, ErasedRenderAssetPlugin, PrepareAssetError,
    },
    render_resource::{Buffer, BufferInitDescriptor, BufferUsages},
    renderer::RenderDevice,
};

use crate::{
    point::{GpuPoint, Point},
    point_cloud::PointCloud,
};

pub struct RenderPointCloud {
    pub buffer: Buffer,
    pub point_count: usize,
    pub properties: Arc<PointCloudProperties>,
}

pub trait PointCloudGpuMapper: Send + Sync + TypePath {
    /// The representation of a point in the "main world" assets
    type Point: Point;
    /// The GPU representation of a point in the "render world"
    type GpuPoint: GpuPoint;

    /// Specifies all ECS data required by [`PointCloudGpuMapper::prepare_asset`].
    ///
    /// For convenience use the [`lifetimeless`](bevy_ecs::system::lifetimeless) [`SystemParam`].
    type Param: SystemParam;

    /// Whether or not to unload the asset after extracting it to the render world.
    #[inline]
    fn asset_usage(_point_cloud: &PointCloud<Self::Point>) -> RenderAssetUsages {
        RenderAssetUsages::default()
    }

    /// Size of the data the asset will upload to the gpu. Specifying a return value
    /// will allow the asset to be throttled via [`RenderAssetBytesPerFrameLimiter`].
    #[inline]
    fn byte_len(point_cloud: &PointCloud<Self::Point>) -> Option<usize> {
        Some(size_of::<Self::GpuPoint>() * point_cloud.points.len())
    }

    fn convert(
        point_cloud: &PointCloud<Self::Point>,
        param: &mut SystemParamItem<Self::Param>,
    ) -> Result<Vec<Self::GpuPoint>, PrepareAssetError<PointCloud<Self::Point>>>;

    /// Low level override to prepare the buffer to be sent
    ///
    /// ECS data may be accessed via `param`.
    #[allow(unused)]
    fn prepare_buffer(
        point_cloud: PointCloud<Self::Point>,
        asset_id: AssetId<PointCloud<Self::Point>>,
        render_device: &RenderDevice,
        param: &mut SystemParamItem<Self::Param>,
    ) -> Result<Buffer, PrepareAssetError<PointCloud<Self::Point>>> {
        let data = Self::convert(&point_cloud, param)?;
        Ok(
            render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("point_cloud_buffer"),
                contents: bytemuck::cast_slice(&data),
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            }),
        )
    }
}

impl<T: GpuPoint> PointCloudGpuMapper for T {
    type Point = T;

    type GpuPoint = T;

    type Param = ();

    fn convert(
        _point_cloud: &PointCloud<Self::Point>,
        _param: &mut SystemParamItem<Self::Param>,
    ) -> Result<Vec<Self::GpuPoint>, PrepareAssetError<PointCloud<Self::Point>>> {
        unreachable!("prepare_buffer doesn't need this method")
    }

    fn prepare_buffer(
        point_cloud: PointCloud<Self::Point>,
        _asset_id: AssetId<PointCloud<Self::Point>>,
        render_device: &RenderDevice,
        _param: &mut SystemParamItem<Self::Param>,
    ) -> Result<Buffer, PrepareAssetError<PointCloud<Self::Point>>> {
        Ok(
            render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("point_cloud_buffer"),
                contents: bytemuck::cast_slice(&point_cloud.points),
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            }),
        )
    }
}

struct ErasedRenderPointCloudAsset<A: PointCloudGpuMapper> {
    _phantom: PhantomData<fn() -> A>,
}

pub struct PointCloudGpuMapperPlugin<
    A: PointCloudGpuMapper,
    AFTER: ErasedRenderAssetDependency + 'static = (),
> {
    phantom: PhantomData<fn() -> (A, AFTER)>,
}

impl<A: PointCloudGpuMapper, AFTER: ErasedRenderAssetDependency + 'static> Default
    for PointCloudGpuMapperPlugin<A, AFTER>
{
    fn default() -> Self {
        Self {
            phantom: Default::default(),
        }
    }
}

impl<A: PointCloudGpuMapper, AFTER: ErasedRenderAssetDependency + 'static> Plugin
    for PointCloudGpuMapperPlugin<A, AFTER>
{
    fn build(&self, app: &mut App) {
        app.add_plugins(ErasedRenderAssetPlugin::<
            ErasedRenderPointCloudAsset<A>,
            AFTER,
        >::default());
    }
}

impl<A: PointCloudGpuMapper> ErasedRenderAsset for ErasedRenderPointCloudAsset<A> {
    type SourceAsset = PointCloud<A::Point>;
    type ErasedAsset = RenderPointCloud;
    type Param = (SRes<RenderDevice>, A::Param);

    fn prepare_asset(
        source_asset: Self::SourceAsset,
        asset_id: AssetId<Self::SourceAsset>,
        (render_device, param): &mut SystemParamItem<Self::Param>,
    ) -> Result<Self::ErasedAsset, PrepareAssetError<Self::SourceAsset>> {
        Ok(RenderPointCloud {
            point_count: source_asset.points.len(),
            buffer: A::prepare_buffer(source_asset, asset_id, render_device, param)?,
            properties: Arc::new(PointCloudProperties {
                vertex_buffer_layout: A::GpuPoint::vertex_buffer_layout(),
                point_cloud_key: ErasedPointCloudKey::new::<A>(),
            }),
        })
    }
}

/// Common [`PointCloud`] properties, calculated for a specific material instance.
#[derive(Default)]
#[allow(clippy::type_complexity)]
pub struct PointCloudProperties {
    pub vertex_buffer_layout: VertexBufferLayout,
    pub point_cloud_key: ErasedPointCloudKey,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ErasedPointCloudKey {
    type_id: TypeId,
}

impl ErasedPointCloudKey {
    pub fn new<A>() -> Self
    where
        A: 'static,
    {
        let type_id = TypeId::of::<A>();
        Self { type_id }
    }
}

impl Default for ErasedPointCloudKey {
    fn default() -> Self {
        Self::new::<()>()
    }
}
