mod simple;
mod standard;

use bevy::{
    color::{Color, ColorToComponents, LinearRgba},
    ecs::{component::Component, query::Changed},
    math::{Vec3, Vec4},
    reflect::{prelude::ReflectDefault, Reflect, ReflectDeserialize, ReflectSerialize},
    render::{extract_component::ExtractComponent, render_resource::*},
};
use bitflags::bitflags;
use serde::{Deserialize, Serialize};
pub use simple::*;
pub use standard::*;

use crate::ViewSettings;

#[derive(Clone, Debug, Copy, Default)]
pub enum RenderMode {
    #[default]
    Normal,
    HighQuality,
    XRay,
    EyeDome,
}

#[derive(Component, Clone, Default, ExtractComponent)]
#[extract_component_filter(Changed<PointCloudViewSettings>)]
pub struct PointCloudViewSettings {
    pub render_mode: RenderMode,
    /// Max 8 clipping planes
    pub clipping_planes: Vec<Vec4>,
    /// Max 8 clipping spheres
    pub clipping_spheres: Vec<Vec4>,
}

#[derive(Clone, Copy, ShaderType, Default)]
pub struct PointCloudViewSettingsUniform {
    pub nb_clipping_planes: u32,
    pub _padding: Vec3,
    pub clipping_planes: [Vec4; 8],
}

bitflags! {
    #[repr(C)]
    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    pub struct PointCloudViewSettingsKey: u64 {
        const RENDER_MODE_0            = 1_u64 <<  0;
        const RENDER_MODE_1            = 1_u64 <<  1;
        const CLIPPING_PLANES          = 1_u64 <<  2;
        const CLIPPING_SPHERES         = 1_u64 <<  3;

        const RENDER_MODE_MASK          = Self::RENDER_MODE_0.bits() | Self::RENDER_MODE_1.bits();

        const RENDER_MODE_NORMAL       = 0;
        const RENDER_MODE_HQ           = Self::RENDER_MODE_0.bits();
        const RENDER_MODE_XRAY         = Self::RENDER_MODE_1.bits();
        const RENDER_MODE_EDL          = Self::RENDER_MODE_0.bits() | Self::RENDER_MODE_1.bits();
    }
}

impl PointCloudViewSettingsKey {
    pub fn render_mode(&self) -> RenderMode {
        match *self & PointCloudViewSettingsKey::RENDER_MODE_MASK {
            PointCloudViewSettingsKey::RENDER_MODE_NORMAL => RenderMode::Normal,
            PointCloudViewSettingsKey::RENDER_MODE_HQ => RenderMode::HighQuality,
            PointCloudViewSettingsKey::RENDER_MODE_XRAY => RenderMode::XRay,
            PointCloudViewSettingsKey::RENDER_MODE_EDL => RenderMode::EyeDome,
            _ => unreachable!("Invalid render mode bits state encountered in pipeline key."),
        }
    }
}

impl From<&RenderMode> for PointCloudViewSettingsKey {
    fn from(value: &RenderMode) -> Self {
        match value {
            RenderMode::Normal => PointCloudViewSettingsKey::RENDER_MODE_NORMAL,
            RenderMode::HighQuality => PointCloudViewSettingsKey::RENDER_MODE_HQ,
            RenderMode::XRay => PointCloudViewSettingsKey::RENDER_MODE_XRAY,
            RenderMode::EyeDome => PointCloudViewSettingsKey::RENDER_MODE_EDL,
        }
    }
}

impl From<&PointCloudViewSettings> for PointCloudViewSettingsKey {
    fn from(settings: &PointCloudViewSettings) -> Self {
        let mut key = PointCloudViewSettingsKey::empty();

        key.insert((&settings.render_mode).into());
        key.set(
            PointCloudViewSettingsKey::CLIPPING_PLANES,
            !settings.clipping_planes.is_empty(),
        );

        key
    }
}

impl ViewSettings for PointCloudViewSettings {
    type Key = PointCloudViewSettingsKey;

    type Data = PointCloudViewSettingsUniform;

    fn pipeline_key(&self) -> Self::Key {
        self.into()
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
