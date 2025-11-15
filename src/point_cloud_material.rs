use bevy_asset::{Asset, AssetId, Handle};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::component::Component;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::{std_traits::ReflectDefault, Reflect};
use bevy_render::render_resource::ShaderType;
use bytemuck::{Pod, Zeroable};

// This is the component that will get passed to the shader
#[derive(Asset, Reflect, ShaderType, Debug, Clone, Default, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct PointCloudMaterial {
    pub point_size: f32,
    pub min_point_size: f32,
    pub max_point_size: f32,
    // WebGL2 structs must be 16 byte aligned.
    #[cfg(all(feature = "webgl", target_arch = "wasm32", not(feature = "webgpu")))]
    pub _webgl2_padding: f32,
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
