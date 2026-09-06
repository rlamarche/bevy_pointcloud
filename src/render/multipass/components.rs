use std::{borrow::Cow, marker::PhantomData};

use bevy::{
    ecs::{component::Component, reflect::ReflectComponent},
    platform::collections::HashMap,
    reflect::{prelude::ReflectDefault, Reflect},
    render::{render_resource::Extent3d, texture::ColorAttachment},
};

use crate::Material;

/// Stores the required textures for each pass, then the textures themselves
#[derive(Component)]
pub struct ViewMultipassTextures<M: Material> {
    pub textures: HashMap<Cow<'static, str>, ColorAttachment>,
    pub size: Extent3d,
    pub _phantom: PhantomData<fn() -> M>,
}

impl<M: Material> Default for ViewMultipassTextures<M> {
    fn default() -> Self {
        Self {
            textures: Default::default(),
            size: Default::default(),
            _phantom: Default::default(),
        }
    }
}
