use bevy::{
    asset::Handle,
    material::labels::{InternedShaderLabel, ShaderLabel},
    render::render_resource::{BlendState, ShaderStages, TextureFormat, TextureSampleType},
    shader::{Shader, ShaderRef},
};
use smallvec::SmallVec;

#[derive(Clone, Debug, Copy)]
pub enum PassKind {
    Geometry,
    Fullscreen,
}

#[derive(Clone, Debug)]
pub enum PassOutput {
    TransientTarget(TransientTarget),
    MainColorTarget,
}

#[derive(Clone, Debug)]
pub struct TransientTarget {
    pub format: TextureFormat,
}

/// Dynamic texture input dependency coming from a previous pass.
#[derive(Clone, Debug)]
pub struct PassInput {
    pub source_pass: usize,
    pub binding_slot: u32,
    pub texture_sample_type: TextureSampleType,
    pub visibility: ShaderStages,
}

pub struct PassShader {
    /// Custom vertex shader handle. If None, uses the default pass vertex pipeline.
    pub vertex: Option<Handle<Shader>>,
    pub fragment: Handle<Shader>,
}

#[derive(Clone, Debug)]
pub enum PassDepth {
    TransientDepth,
    ViewDepth,
}

pub struct PassDescriptor<PassId> {
    pub id: PassId,
    pub kind: PassKind,
    pub label: &'static str,
}

pub struct GeometryPassParams {
    pub vertex_shader: ShaderRef,
    pub fragment_shader: ShaderRef,
    pub inputs: Vec<PassInput>,
    pub output: PassOutput,
    // pub depth: PassDepth,
    pub blend: Option<BlendState>,
    pub depth_write_enabled: Option<bool>,
}

pub struct FullscreenPassParams {
    pub vertex_shader: ShaderRef,
    pub fragment_shader: ShaderRef,
    pub inputs: Vec<PassInput>,
    pub output: PassOutput,
    // pub depth: PassDepth,
    pub blend: Option<BlendState>,
    pub depth_write_enabled: Option<bool>,
}

pub struct PassProperties {
    pub label: &'static str,
    pub kind: PassKind,
    /// Backing array is a size of 3 because the [`StandardMaterial`](https://docs.rs/bevy/latest/bevy/pbr/struct.StandardMaterial.html)
    /// has 3 custom shaders (`frag`, `prepass_frag`, `deferred_frag`) which is the
    /// most common use case
    pub shaders: SmallVec<[(InternedShaderLabel, Handle<Shader>); 3]>,

    pub inputs: Vec<PassInput>,
    pub output: PassOutput,
    pub blend: Option<BlendState>,
    pub depth_write_enabled: Option<bool>,
}

impl PassProperties {
    pub fn get_shader(&self, label: impl ShaderLabel) -> Option<Handle<Shader>> {
        self.shaders
            .iter()
            .find(|(inner_label, _)| inner_label == &label.intern())
            .map(|(_, shader)| shader)
            .cloned()
    }

    pub fn add_shader(&mut self, label: impl ShaderLabel, shader: Handle<Shader>) {
        self.shaders.push((label.intern(), shader));
    }
}
