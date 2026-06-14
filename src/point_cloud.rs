use std::marker::PhantomData;

use bevy_app::prelude::*;
use bevy_asset::{AsAssetId, Asset, AssetApp, AssetId, Handle};
use bevy_camera::visibility::{add_visibility_class, Visibility, VisibilityClass};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::component::Component;
use bevy_reflect::TypePath;
use bevy_transform::prelude::*;

use crate::{point::{GpuPoint, Point}, render::RenderPipelinePlugin};

pub const QUAD_POSITIONS: &[[f32; 3]] = &[
    [-0.5, -0.5, 0.0],
    [0.5, -0.5, 0.0],
    [0.5, 0.5, 0.0],
    [-0.5, 0.5, 0.0],
];
pub const QUAD_INDICES: &[u32] = &[0, 1, 2, 2, 3, 0];

#[derive(Debug, Clone, Asset, TypePath)]
pub struct PointCloud<T: Point> {
    pub points: Vec<T>,
}

#[derive(Component, Clone, Debug, Default, Deref, DerefMut, TypePath, PartialEq, Eq)]
#[require(Transform)]
pub struct PointCloud3d<T: Point>(pub Handle<PointCloud<T>>);

impl<T: Point> From<PointCloud3d<T>> for AssetId<PointCloud<T>> {
    fn from(point_cloud: PointCloud3d<T>) -> Self {
        point_cloud.id()
    }
}

impl<T: Point> From<&PointCloud3d<T>> for AssetId<PointCloud<T>> {
    fn from(pointcloud: &PointCloud3d<T>) -> Self {
        pointcloud.id()
    }
}

impl<T: Point> AsAssetId for PointCloud3d<T> {
    type Asset = PointCloud<T>;

    fn as_asset_id(&self) -> AssetId<Self::Asset> {
        self.id()
    }
}

pub struct PointCloudPlugin<T: Point, U: GpuPoint>(PhantomData<fn() -> (T, U)>)
where
    for<'a> &'a T: Into<U>;

impl<T: Point, U: GpuPoint> Default for PointCloudPlugin<T, U>
where
    for<'a> &'a T: Into<U>,
{
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: Point, U: GpuPoint> Plugin for PointCloudPlugin<T, U>
where
    for<'a> &'a T: Into<U>,
{
    fn build(&self, app: &mut App) {
        app.register_required_components::<PointCloud3d<T>, Visibility>()
            .register_required_components::<PointCloud3d<T>, VisibilityClass>();

        app.init_asset::<PointCloud<T>>();
        // .init_asset::<PointCloudMaterial>()
        // .register_asset_reflect::<PointCloudMaterial>();
        app.add_plugins(RenderPipelinePlugin::<T, U>::default());

        app.world_mut()
            .register_component_hooks::<PointCloud3d<T>>()
            .on_add(add_visibility_class::<PointCloud3d<T>>);
    }
}
