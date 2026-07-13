mod simple;
mod standard;

use bevy::{
    color::{Color, ColorToComponents, LinearRgba},
    math::Vec4,
    reflect::{prelude::ReflectDefault, Reflect},
    render::render_resource::ShaderType,
};
pub use simple::*;
pub use standard::*;

#[derive(Debug, Copy, Clone, PartialEq, Reflect)]
#[reflect(Default, PartialEq, Debug)]
#[cfg_attr(
    feature = "serialize",
    derive(serde::Serialize, serde::Deserialize),
    reflect(Serialize, Deserialize)
)]
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
