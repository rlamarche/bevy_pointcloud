mod rgb;

use bevy_math::prelude::*;
use bevy_reflect::TypePath;
pub use rgb::*;

pub trait Point: Clone + Sync + Send + TypePath {
    fn position(&self) -> &Vec3;
}
