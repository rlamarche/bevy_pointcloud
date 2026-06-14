use bevy_app::Plugin;
use bevy_asset::{load_internal_asset, uuid_handle, Asset, Handle};
use bevy_reflect::TypePath;
use bevy_render::render_resource::AsBindGroup;
use bevy_shader::{Shader, load_shader_library};

use crate::{point::RGBPoint, PointCloudMaterial, PointCloudMaterialPlugin, RenderPass};

const VERTEX_SHADER_HANDLE: Handle<Shader> = uuid_handle!("7664491d-3246-4c96-b716-23786e9d0eb2");
const FRAGMENT_SHADER_HANDLE: Handle<Shader> = uuid_handle!("a0280d36-f841-42ec-a730-ad1b24e54bca");
const NORMALIZE_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("9b3e55ab-e9ab-4d88-9725-40dfb9c7f046");

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

impl PointCloudMaterial for SimplePointCloudMaterial {
    fn vertex_shader(_: RenderPass) -> bevy_shader::ShaderRef {
        VERTEX_SHADER_HANDLE.into()
    }

    fn fragment_shader(_: RenderPass) -> bevy_shader::ShaderRef {
        FRAGMENT_SHADER_HANDLE.into()
    }

    fn shader_defs(pass: RenderPass) -> Vec<bevy_shader::ShaderDefVal> {
        match pass {
            RenderPass::Depth => vec!["DEPTH_PASS".into(), "HQ_DEPTH_PASS".into()],
            RenderPass::Attribute => vec!["ATTRIBUTE_PASS".into(), "WEIGHTED_SPLATS".into()],
        }
    }
}

pub struct SimplePointCloudMaterialPlugin;

impl Plugin for SimplePointCloudMaterialPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        load_shader_library!(app, "binding.wgsl");
        load_internal_asset!(app, VERTEX_SHADER_HANDLE, "vertex.wgsl", Shader::from_wgsl);
        load_internal_asset!(
            app,
            FRAGMENT_SHADER_HANDLE,
            "fragment.wgsl",
            Shader::from_wgsl
        );
        load_internal_asset!(
            app,
            NORMALIZE_SHADER_HANDLE,
            "normalize.wgsl",
            Shader::from_wgsl
        );
        app.add_plugins(PointCloudMaterialPlugin::<
            RGBPoint,
            RGBPoint,
            SimplePointCloudMaterial,
        >::default());
    }
}
