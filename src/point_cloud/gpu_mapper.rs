use std::{any::TypeId, hash::Hash, marker::PhantomData, sync::Arc};

use bevy_app::prelude::*;
use bevy_asset::prelude::*;
use bevy_ecs::{
    prelude::*,
    system::{lifetimeless::SRes, SystemParamItem},
};
use bevy_mesh::VertexBufferLayout;
use bevy_reflect::TypePath;
use bevy_render::{
    erased_render_asset::{
        ErasedRenderAsset, ErasedRenderAssetDependency, ErasedRenderAssetPlugin, PrepareAssetError,
    },
    render_resource::{encase::private::Length, Buffer, BufferInitDescriptor, BufferUsages},
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
    type Point: Point;
    type GpuPoint: GpuPoint;

    /// Defines how the component is transferred into the "render world".
    fn convert(point_cloud: PointCloud<Self::Point>) -> Vec<Self::GpuPoint>;
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
    type Param = SRes<RenderDevice>;

    fn prepare_asset(
        source_asset: Self::SourceAsset,
        _asset_id: AssetId<Self::SourceAsset>,
        render_device: &mut SystemParamItem<Self::Param>,
    ) -> Result<Self::ErasedAsset, PrepareAssetError<Self::SourceAsset>> {
        let points = A::convert(source_asset);
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("PointCloud data buffer"),
            contents: bytemuck::cast_slice(points.as_slice()),
            usage: BufferUsages::VERTEX, // | BufferUsages::COPY_DST,
        });
        bevy_log::info!("Created erased buffer");
        Ok(RenderPointCloud {
            buffer,
            point_count: points.length(),
            properties: Arc::new(PointCloudProperties {
                vertex_buffer_layout: A::GpuPoint::vertex_buffer_layout(),
                pointcloud_key: ErasedPointCloudKey::new::<A>(),
            }),
        })
    }
}

/// Common [`PointCloud`] properties, calculated for a specific material instance.
#[derive(Default)]
#[allow(clippy::type_complexity)]
pub struct PointCloudProperties {
    pub vertex_buffer_layout: VertexBufferLayout,
    pub pointcloud_key: ErasedPointCloudKey,
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
