mod simple;
mod standard;

use bevy::{
    color::{Color, ColorToComponents, LinearRgba},
    ecs::{component::Component, query::Changed},
    math::{Vec3, Vec4},
    reflect::{prelude::ReflectDefault, Reflect, ReflectDeserialize, ReflectSerialize},
    render::{extract_component::ExtractComponent, render_resource::*},
};
use serde::{Deserialize, Serialize};
pub use simple::*;
pub use standard::*;

use crate::ViewSettings;

#[derive(Component, Clone, Default, ExtractComponent)]
#[extract_component_filter(Changed<PointCloudViewSettings>)]
pub struct PointCloudViewSettings {
    pub clipping_planes: Vec<Vec4>,
}

#[derive(Clone, Copy, ShaderType, Default)]
pub struct PointCloudViewSettingsUniform {
    pub nb_clipping_planes: u32,
    pub _padding: Vec3,
    pub clipping_planes: [Vec4; 8],
}

impl ViewSettings for PointCloudViewSettings {
    type Key = u8;

    type Data = PointCloudViewSettingsUniform;

    fn pipeline_key(&self) -> Self::Key {
        self.clipping_planes.len() as u8
    }

    fn to_data(&self) -> Self::Data {
        PointCloudViewSettingsUniform {
            nb_clipping_planes: self.clipping_planes.len() as u32,
            _padding: Default::default(),
            clipping_planes: std::array::from_fn(|i| {
                self.clipping_planes.get(i).copied().unwrap_or_default()
            }),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Default, PartialEq, Debug, Serialize, Deserialize)]
pub struct ColorStop {
    /// Color
    pub color: Color,
    /// Normalized position of the stop (between 0 and 1).
    pub point: f32,
}

impl Default for ColorStop {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            point: 0.5,
        }
    }
}

/// The GPU representation of the uniform data of a [`ColorStop`].
#[derive(Clone, Default, ShaderType)]
pub struct ColorStopUniform {
    /// Color
    pub color: Vec4,
    /// Normalized position of the stop (between 0 and 1).
    pub point: f32,
}

impl From<ColorStop> for ColorStopUniform {
    fn from(value: ColorStop) -> Self {
        ColorStopUniform {
            color: LinearRgba::from(value.color).to_vec4(),
            point: value.point,
        }
    }
}
