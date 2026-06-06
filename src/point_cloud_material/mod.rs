mod simple;

use std::marker::PhantomData;

use bevy_app::prelude::*;
use bevy_asset::{Asset, AssetApp, AssetId, Handle};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::{component::Component, reflect::ReflectComponent};
use bevy_reflect::{std_traits::ReflectDefault, Reflect};
use bevy_render::render_resource::AsBindGroup;
use bevy_shader::ShaderRef;
use derive_more::derive::From;
pub use simple::*;

use crate::{
    point::{GpuPoint, Point},
    render::RenderPipelinePlugin,
};

pub trait PointCloudMaterial: Asset + AsBindGroup + Clone + Sized {
    /// Returns this material's vertex shader. If [`ShaderRef::Default`] is returned, the default mesh vertex shader
    /// will be used.
    fn vertex_shader() -> ShaderRef {
        ShaderRef::Default
    }

    /// Returns this material's fragment shader. If [`ShaderRef::Default`] is returned, the default mesh fragment shader
    /// will be used.
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Default
    }
}

#[derive(Component, Clone, Debug, Deref, DerefMut, Reflect, From)]
#[reflect(Component, Default, Clone, PartialEq)]
pub struct PointCloudMaterial3d<M: PointCloudMaterial>(pub Handle<M>);

impl<M: PointCloudMaterial> Default for PointCloudMaterial3d<M> {
    fn default() -> Self {
        Self(Handle::default())
    }
}

impl<M: PointCloudMaterial> PartialEq for PointCloudMaterial3d<M> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<M: PointCloudMaterial> Eq for PointCloudMaterial3d<M> {}

impl<M: PointCloudMaterial> From<PointCloudMaterial3d<M>> for AssetId<M> {
    fn from(point_cloud_material_3d: PointCloudMaterial3d<M>) -> Self {
        point_cloud_material_3d.id()
    }
}

impl<M: PointCloudMaterial> From<&PointCloudMaterial3d<M>> for AssetId<M> {
    fn from(point_cloud_material_3d: &PointCloudMaterial3d<M>) -> Self {
        point_cloud_material_3d.id()
    }
}

#[allow(clippy::type_complexity)]
pub struct PointCloudMaterialPlugin<T: Point, U: GpuPoint, M: PointCloudMaterial>(
    PhantomData<fn() -> (T, U, M)>,
)
where
    for<'a> &'a T: Into<U>;

impl<T: Point, U: GpuPoint, M: PointCloudMaterial> Default for PointCloudMaterialPlugin<T, U, M>
where
    for<'a> &'a T: Into<U>,
{
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: Point, U: GpuPoint, M: PointCloudMaterial> Plugin for PointCloudMaterialPlugin<T, U, M>
where
    for<'a> &'a T: Into<U>,
{
    fn build(&self, app: &mut App) {
        app.init_asset::<M>();
        app.add_plugins(RenderPipelinePlugin::<T, U, M>::default());
    }
}
