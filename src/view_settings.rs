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
#[extract_component_filter(Changed<PointCloudEmptyViewSettings>)]
pub struct PointCloudEmptyViewSettings;

#[derive(ShaderType, Default)]
pub struct PointCloudEmptyViewSettingsUniform {
    pub _padding: Vec4,
}

impl ViewSettings for PointCloudEmptyViewSettings {
    type Key = ();
    type Data = PointCloudEmptyViewSettingsUniform;

    fn pipeline_key(&self) -> Self::Key {}
    fn to_data(&self) -> Self::Data {
        Self::Data::default()
    }
}
