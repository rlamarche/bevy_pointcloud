use bevy_asset::Asset;
use bevy_reflect::TypePath;
use bevy_render::render_resource::AsBindGroup;

use crate::PointCloudMaterial;

// This is the component that will get passed to the shader
#[derive(Asset, Debug, Clone, AsBindGroup, TypePath, Default)]
#[repr(C)]
pub struct SimplePointCloudMaterial {
    #[uniform(0)]
    pub point_size: f32,
    #[uniform(0)]
    pub min_point_size: f32,
    #[uniform(0)]
    pub max_point_size: f32,
    // WebGL2 structs must be 16 byte aligned.
    #[cfg(all(feature = "webgl", target_arch = "wasm32", not(feature = "webgpu")))]
    pub _webgl2_padding: f32,
}

impl PointCloudMaterial for SimplePointCloudMaterial {}
