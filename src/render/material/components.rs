use bevy::{ecs::component::Component, render::render_resource::UniformBuffer};

use crate::{render::PointCloudMaterial, ViewSettings};

#[derive(Component)]
pub struct PreparedViewSettingsUniform<M: PointCloudMaterial> {
    pub view_settings_uniform_buffer: UniformBuffer<<M::ViewSettings as ViewSettings>::Data>,
}

impl<M: PointCloudMaterial> Default for PreparedViewSettingsUniform<M> {
    fn default() -> Self {
        Self {
            view_settings_uniform_buffer: Default::default(),
        }
    }
}
