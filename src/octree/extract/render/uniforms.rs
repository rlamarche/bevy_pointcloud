use bevy_render::render_resource::ShaderType;
use bytemuck::{Pod, Zeroable};

#[derive(ShaderType, Pod, Zeroable, Clone, Copy, Default)]
#[repr(C)]
pub struct OctreeEntityUniform {
    pub octree_index: u32,
    // WebGL2 structs must be 16 byte aligned.
    #[cfg(all(feature = "webgl", target_arch = "wasm32", not(feature = "webgpu")))]
    pub _webgl2_padding: Vec3,
}
