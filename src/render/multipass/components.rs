use std::{borrow::Cow, marker::PhantomData};

use bevy::{
    ecs::component::Component,
    platform::collections::HashMap,
    prelude::{Deref, DerefMut},
    render::{
        render_resource::{BindGroup, Extent3d, TextureView},
        sync_world::MainEntityHashMap,
        texture::{CachedTexture, ColorAttachment},
    },
};

use crate::Material;

/// Stores the required textures for each pass, then the textures themselves
#[derive(Component, Deref, DerefMut)]
pub struct ViewMultipassTextures<M: Material> {
    #[deref]
    pub textures: HashMap<Cow<'static, str>, MultipassTexture>,
    pub size: Extent3d,
    pub has_changed: bool,
    pub _phantom: PhantomData<fn() -> M>,
}

#[derive(Clone)]
pub struct MultipassTexture {
    pub color_attachment: ColorAttachment,
    pub texture: CachedTexture,
}

impl<M: Material> Default for ViewMultipassTextures<M> {
    fn default() -> Self {
        Self {
            textures: Default::default(),
            size: Default::default(),
            has_changed: true,
            _phantom: Default::default(),
        }
    }
}

/// Stores bind groups like [`crate::ViewPointCloudBindGroups`] but for each material/pass so
/// specific pass groups can be provided.
#[derive(Component, Clone)]
pub struct ViewPointCloudPassBindGroup<M: Material, const PASS: usize> {
    // contains one bind group per point cloud
    pub bind_groups: MainEntityHashMap<BindGroup>,
    _phantom: PhantomData<M>,
}

impl<M: Material, const PASS: usize> Default for ViewPointCloudPassBindGroup<M, PASS> {
    fn default() -> Self {
        Self {
            bind_groups: Default::default(),
            _phantom: PhantomData,
        }
    }
}
