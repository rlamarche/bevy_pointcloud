mod gpu_mapper;
mod resources;
use std::{marker::PhantomData, sync::Arc};

use bevy_app::prelude::*;
use bevy_asset::{AsAssetId, Asset, AssetApp, AssetId, Handle};
use bevy_camera::{
    primitives::Aabb,
    visibility::{add_visibility_class, Visibility, VisibilityClass},
};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use bevy_reflect::TypePath;
use bevy_transform::prelude::*;
pub use gpu_mapper::*;
pub use resources::*;

use crate::{point::Point, render::RenderPipelinePlugin};

pub const QUAD_POSITIONS: &[[f32; 3]] = &[
    [-0.5, -0.5, 0.0],
    [0.5, -0.5, 0.0],
    [0.5, 0.5, 0.0],
    [-0.5, 0.5, 0.0],
];
pub const QUAD_INDICES: &[u32] = &[0, 1, 2, 2, 3, 0];

#[derive(Debug, Clone, Asset, TypePath)]
pub struct PointCloud<T: Point> {
    pub points: Arc<Vec<T>>,
    pub aabb: Option<Aabb>,
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

pub struct PointCloudsPlugin<T: Point>(PhantomData<fn() -> T>);

impl<T: Point> Default for PointCloudsPlugin<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: Point> Plugin for PointCloudsPlugin<T> {
    fn build(&self, app: &mut App) {
        app.init_asset::<PointCloud<T>>()
            .register_required_components::<PointCloud3d<T>, Visibility>()
            .register_required_components::<PointCloud3d<T>, VisibilityClass>()
            .add_plugins(RenderPipelinePlugin::<T>::default());

        app.world_mut()
            .register_component_hooks::<PointCloud3d<T>>()
            .on_add(add_visibility_class::<PointCloud3d<T>>);
    }
}
