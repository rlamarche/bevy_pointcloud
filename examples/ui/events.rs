use bevy::{ecs::event::Event, prelude::Deref};
use bevy_pointcloud::SplatOrientation;

#[derive(Event)]
pub struct UiHoveringEvent(pub bool);

#[derive(Event)]
pub struct UiDraggingEvent(pub bool);

#[derive(Event)]
pub struct UiClickDespawnEvent;

#[derive(Event)]
pub struct UiClickSpawnEvent;

#[derive(Event, Deref)]
pub struct UiClickSetSplatOrientation(pub SplatOrientation);
