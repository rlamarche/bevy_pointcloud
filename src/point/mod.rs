mod rgb;

use bevy_math::prelude::*;
use bevy_reflect::TypePath;
use bytemuck::{Pod, Zeroable};
pub use rgb::*;

pub trait Point: Clone + Sync + Send + TypePath {
    fn position(&self) -> &Vec3;
}

pub trait GpuPoint: Sync + Send + Pod + Zeroable + Copy + TypePath {}
