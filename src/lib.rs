use std::marker::PhantomData;

use bevy_app::prelude::*;
use bevy_asset::AssetApp;
use bevy_camera::visibility::{add_visibility_class, Visibility, VisibilityClass};

use crate::{
    point::Point,
    point_cloud::{PointCloud, PointCloud3d},
    point_cloud_material::PointCloudMaterial,
    render::point_cloud::GpuPoint,
};

pub mod bevy;
pub mod loader;
#[cfg(feature = "octree")]
pub mod octree;
#[cfg(feature = "octree")]
pub mod octree_loader;
pub mod point;
pub mod point_cloud;
pub mod point_cloud_material;
#[cfg(feature = "pointcloud_octree")]
pub mod pointcloud_octree;
pub mod prelude;
pub mod render;

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

        app.init_asset::<PointCloud<T>>()
            .init_asset::<PointCloudMaterial>()
            .register_asset_reflect::<PointCloudMaterial>();
        app.add_plugins(render::RenderPipelinePlugin::<T, U>::default());

        app.world_mut()
            .register_component_hooks::<PointCloud3d<T>>()
            .on_add(add_visibility_class::<PointCloud3d<T>>);
    }
}
