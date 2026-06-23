use std::{any::TypeId, marker::PhantomData, sync::Arc};

use bevy_app::prelude::*;
use bevy_asset::prelude::*;
use bevy_camera::primitives::Aabb;
use bevy_ecs::{
    prelude::*,
    system::{lifetimeless::Read, SystemParamItem},
};

use crate::{
    point::GpuPoint,
    point_cloud::{
        ErasedPointCloudKey, PointCloudGpuMapper, PointCloudGpuMapperKey, PointCloudProperties,
    },
    point_cloud_material::{PointCloudMaterial, PointCloudMaterial3d},
    pointcloud_octree::{asset::PointCloudOctree, component::PointCloudOctree3d},
    render_asset::{
        ErasedRenderAssetComponent, ErasedRenderAssetComponentPlugin, PrepareAssetComponentError,
    },
};

#[derive(Default)]
pub struct RenderPointCloudOctree {
    pub point_count: usize,
    pub bounding_box: Aabb,
    pub properties: Arc<PointCloudProperties>,
}

pub struct ErasedPointCloudOctreeGpuMapper<A: PointCloudGpuMapper, C: Component>(
    PhantomData<fn() -> (A, C)>,
);

impl<A: PointCloudGpuMapper, C: Component> ErasedRenderAssetComponent
    for ErasedPointCloudOctreeGpuMapper<A, C>
{
    type SourceAsset = PointCloudOctree<A::Point>;
    type ExtractedAsset = RenderPointCloudOctree;
    type ErasedAsset = RenderPointCloudOctree;
    type Param = ();

    type QueryData = Read<PointCloudOctree3d<A::Point>>;
    type QueryFilter = With<C>;

    type Key = PointCloudGpuMapperKey;

    fn extract_asset(source_asset: &Self::SourceAsset) -> Self::ExtractedAsset {
        let Some(root_node) = source_asset.node_root() else {
            return Default::default();
        };

        RenderPointCloudOctree {
            point_count: 0, // TODO
            bounding_box: root_node.hierarchy.bounding_box,
            properties: Arc::new(PointCloudProperties {
                vertex_buffer_layout: A::GpuPoint::vertex_buffer_layout(),
                point_cloud_key: ErasedPointCloudKey::new::<A>(),
            }),
        }
    }

    fn prepare_asset(
        extracted_asset: Self::ExtractedAsset,
        _asset_id: AssetId<Self::SourceAsset>,
        _type_id: TypeId,
        _: &mut SystemParamItem<Self::Param>,
    ) -> Result<Self::ErasedAsset, PrepareAssetComponentError<Self::ExtractedAsset>> {
        Ok(extracted_asset)
    }

    fn asset_id(data: bevy_ecs::query::ROQueryItem<Self::QueryData>) -> AssetId<Self::SourceAsset> {
        data.into()
    }
}

pub struct PointCloudOctreeGpuMapperPlugin<M: PointCloudMaterial, A: PointCloudGpuMapper>(
    PhantomData<fn() -> (M, A)>,
);

impl<M: PointCloudMaterial, A: PointCloudGpuMapper> Default
    for PointCloudOctreeGpuMapperPlugin<M, A>
{
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<M: PointCloudMaterial, A: PointCloudGpuMapper> Plugin
    for PointCloudOctreeGpuMapperPlugin<M, A>
{
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins(ErasedRenderAssetComponentPlugin::<
            ErasedPointCloudOctreeGpuMapper<A, PointCloudMaterial3d<M, A>>,
        >::default());
    }
}
