use std::hash::Hash;

use bevy::{
    ecs::{component::Component, query::Changed},
    math::Vec4,
    render::{
        extract_component::ExtractComponent,
        render_resource::{encase::private::WriteInto, ShaderType},
    },
};

pub trait ViewSettings: Clone + Component + ExtractComponent + Default {
    type Key: Hash + Eq + PartialEq + Clone + Send + Sync + 'static;
    type Data: Send + Sync + 'static + ShaderType + WriteInto + Default;

    fn pipeline_key(&self) -> Self::Key;
    fn to_data(&self) -> Self::Data;
}

#[derive(Component, ExtractComponent, Clone, Default)]
#[extract_component_filter(Changed<EmptyViewSettings>)]
pub struct EmptyViewSettings;

#[derive(ShaderType, Default)]
pub struct EmptyViewSettingsUniform {
    pub _padding: Vec4,
}

impl ViewSettings for EmptyViewSettings {
    type Key = ();
    type Data = EmptyViewSettingsUniform;

    fn pipeline_key(&self) -> Self::Key {}
    fn to_data(&self) -> Self::Data {
        Self::Data::default()
    }
}
