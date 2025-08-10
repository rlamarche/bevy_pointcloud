use bevy_derive::Deref;
use bevy_ecs::{prelude::*, query::QueryItem};
use bevy_math::prelude::*;
use bevy_render::{
    extract_component::ExtractComponent, render_resource::*,
};
use bytemuck::{Pod, Zeroable};
use std::sync::Arc;

#[derive(Component, Deref)]
pub struct PointCloudInstance(pub Arc<Vec<PointCloudData>>);

impl ExtractComponent for PointCloudInstance {
    type QueryData = &'static PointCloudInstance;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<'_, Self::QueryData>) -> Option<Self> {
        Some(PointCloudInstance(item.0.clone()))
    }
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct PointCloudData {
    pub position: Vec3,
    pub point_size: f32,
    pub color: [f32; 4],
}

#[derive(Component)]
pub struct GpuPointCloudData {
    pub buffer: Buffer,
    pub length: usize,
}
