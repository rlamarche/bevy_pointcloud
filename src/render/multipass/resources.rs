use std::marker::PhantomData;

use bevy::{asset::Handle, ecs::resource::Resource, shader::Shader};

use crate::render::PointCloudMaterial;

#[derive(Resource, Clone)]
pub struct PreparedPointCloudFullscreenPass<M: PointCloudMaterial, const PASS: usize> {
    pub fragment_shader: Handle<Shader>,
    pub vertex_shader: Option<Handle<Shader>>,
    pub _phantom: PhantomData<M>,
}
