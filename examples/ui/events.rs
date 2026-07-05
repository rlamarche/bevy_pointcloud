use bevy::ecs::event::Event;

#[derive(Event)]
pub struct UiHoveringEvent(pub bool);

#[derive(Event)]
pub struct UiDraggingEvent(pub bool);
