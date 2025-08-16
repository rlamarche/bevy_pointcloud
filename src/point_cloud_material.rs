use bevy_asset::{Asset, AssetId, Handle};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::component::Component;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{std_traits::ReflectDefault, Reflect};
use bevy_render::render_resource::AsBindGroup;

// This is the component that will get passed to the shader
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct PointCloudMaterial {
    #[uniform(0)]
    pub point_size: f32,
}

#[derive(Component, Clone, Debug, Default, Deref, DerefMut, Reflect, PartialEq, Eq)]
#[reflect(Component, Default, Clone, PartialEq)]
pub struct PointCloudMaterial3d(pub Handle<PointCloudMaterial>);

impl From<PointCloudMaterial3d> for AssetId<PointCloudMaterial> {
    fn from(point_cloud_material_3d: PointCloudMaterial3d) -> Self {
        point_cloud_material_3d.id()
    }
}

impl From<&PointCloudMaterial3d> for AssetId<PointCloudMaterial> {
    fn from(point_cloud_material_3d: &PointCloudMaterial3d) -> Self {
        point_cloud_material_3d.id()
    }
}
