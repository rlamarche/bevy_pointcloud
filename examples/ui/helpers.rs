use std::collections::VecDeque;

use bevy::{
    feathers::{
        controls::{FeathersCheckbox, FeathersSlider},
        theme::{ThemeBackgroundColor, ThemedText},
        tokens,
    },
    log::info,
    picking::prelude::*,
    prelude::*,
    ui::Checked,
    ui_widgets::{
        checkbox_self_update, observe, slider_self_update, SliderPrecision, SliderStep, ValueChange,
    },
};

use crate::ui::SettingsPanel;

use super::{DraggablePanel, UiDraggingEvent, UiHoveringEvent, UiSettings};

// ---------------------------------------------------------------------------
// UI Components
// ---------------------------------------------------------------------------

pub fn settings_panel() -> impl Scene {
    bsn! {
        SettingsPanel
        DraggablePanel
        Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            justify_content: JustifyContent::Start,
            padding: UiRect::all(px(8)),
            row_gap: px(8),
            width: percent(30),
            min_width: px(200),
            border_radius: BorderRadius::all(Val::Px(10.0)),
        }
        ThemeBackgroundColor(tokens::WINDOW_BG)
        ui_drag_observer()
        ui_hover_observer()
        Children [
            settings_title(),
            // settings_checkbox(),
            settings_section(
                "EDL radius",
                settings_slider(0.0, 10.0, 1.4, 0.01, 2, |ui_settings, value| {
                    ui_settings.edl_radius = value;
                })
            ),
            settings_section(
                "EDL strength",
                settings_slider(0.0, 10.0, 0.4, 0.01, 2, |ui_settings, value| {
                    ui_settings.edl_strength = value;
                })
            ),
            settings_section(
                "EDL neighbour count",
                settings_slider(4.0, 8.0, 4.0, 4.0, -1, |ui_settings, value| {
                    ui_settings.edl_neighbour_count = value as u32;
                })
            ),
            settings_section(
                "Min Node Size",
                settings_slider(30.0, 1000.0, 150.0, 10.0, 0, |ui_settings, value| {
                    ui_settings.min_node_size = value;
                })
            ),
            settings_section(
                "Point Budget",
                settings_slider(
                    100_000.0,
                    100_000_000.0,
                    10_000_000.0,
                    100.0,
                    -2,
                    |ui_settings, value| {
                        ui_settings.point_budget = value as usize;
                    }
                )
            ),
        ]
    }
}

fn settings_checkbox() -> impl Scene {
    bsn! {
        @FeathersCheckbox {
            @caption: bsn! { Text("Checkbox") ThemedText }
        }
        Checked
        AccessibleLabel("Checkbox Example")
        on(
            |change: On<ValueChange<bool>>| {
                info!("Checkbox clicked: {}", change.value);
            }
        )
        on(checkbox_self_update)
        on(prevent_drag_parent)
    }
    // (
    //     checkbox(Checked, Spawn((Text::new("Checkbox"), ThemedText))),
    //     observe(|_change: On<ValueChange<bool>>, mut _commands: Commands| {
    //         info!("Checkbox clicked!");
    //     }),
    //     observe(checkbox_self_update),
    //     observe(prevent_drag_parent),
    // )
}

fn settings_slider(
    min: f32,
    max: f32,
    value: f32,
    step: f32,
    precision: i32,
    callback: fn(settings: &mut UiSettings, value: f32) -> (),
) -> impl Scene {
    bsn! {
        @FeathersSlider {
            @min: min,
            @max: max,
            @value: value,
            // @step: step,
            // @precision: precision,
        }
        SliderStep(step)
        SliderPrecision(precision)
        on(
            move |change: On<ValueChange<f32>>, mut ui_settings: ResMut<UiSettings>| {
                callback(&mut ui_settings, change.value);
            },
        )
        on(slider_self_update)
        ui_drag_observer()
    }
}

// ---------------------------------------------------------------------------
// UI Helpers
// ---------------------------------------------------------------------------

fn settings_title() -> impl Scene {
    bsn! {
        Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center, // horizontal
            align_items: AlignItems::Center,         // vertical
        }
        Children [
            (Text("Settings"))
        ]
        drag_handle()
    }
}

fn settings_section(title: &str, content: impl Scene) -> impl Scene {
    bsn! {
        Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
        }
        Children [
            settings_section_title(title),
            content,
        ]
    }
}

fn settings_section_title(title: &str) -> impl Scene {
    bsn! {
        Text(title)
        TextFont {
            font_size: FontSize::Px(12.0)
        }
    }
}

// ---------------------------------------------------------------------------
// Behavior helpers
// ---------------------------------------------------------------------------

// fn pointer_over_trigger(_: On<Pointer<Over>>, mut commands: Commands) {
//     commands.trigger(UiHoveringEvent(true));
// }

// fn pointer_out_trigger(_: On<Pointer<Out>>, mut commands: Commands) {
//     commands.trigger(UiHoveringEvent(false));
// }

// fn drag_start_trigger(_: On<Pointer<DragStart>>, mut commands: Commands) {
//     commands.trigger(UiDraggingEvent(true));
// }

// fn drag_end_trigger(_: On<Pointer<DragEnd>>, mut commands: Commands) {
//     commands.trigger(UiDraggingEvent(false));
// }

fn ui_hover_observer() -> impl Scene {
    bsn! {
        on(|_: On<Pointer<Over>>, mut commands: Commands| {
            commands.trigger(UiHoveringEvent(true));
        })
        on(|_: On<Pointer<Out>>, mut commands: Commands| {
            commands.trigger(UiHoveringEvent(false));
        })
    }
}

/// Sends dragging event state to ui state to prevent camera movements while dragging
fn ui_drag_observer() -> impl Scene {
    bsn! {
        on(|_: On<Pointer<DragStart>>, mut commands: Commands| {
            commands.trigger(UiDraggingEvent(true));
        })
        on(|_: On<Pointer<DragEnd>>, mut commands: Commands| {
            commands.trigger(UiDraggingEvent(false));
        })
    }
}

/// Returns an ancestor having a specific component
fn find_ancestor<C: Component>(
    mut entity: Entity,
    parent_query: Query<(&ChildOf, Has<C>)>,
) -> Option<Entity> {
    while let Ok((ChildOf(parent), has_marker)) = parent_query.get(entity) {
        if has_marker {
            return Some(entity);
        }
        entity = *parent;
    }

    None
}

/// Returns the first descendant found having a specific component
fn find_descendant<C: Component>(
    entity: Entity,
    parent_query: Query<(Option<&Children>, Has<C>)>,
) -> Option<Entity> {
    let mut stack = VecDeque::new();
    stack.push_back(entity);

    while let Some(entity) = stack.pop_front() {
        while let Ok((children, has_marker)) = parent_query.get(entity) {
            if has_marker {
                return Some(entity);
            }
            if let Some(children) = children {
                for child in children {
                    stack.push_back(*child);
                }
            }
        }
    }

    None
}

fn drag_handle() -> impl Scene {
    bsn! {
        on(
        |trigger: On<Pointer<Drag>>,
         mut query: Query<&mut UiTransform>,
         parent_query: Query<(&ChildOf, Has<DraggablePanel>)>| {
            if let Some(panel_entity) = find_ancestor(trigger.entity, parent_query)
                && let Ok(mut transform) = query.get_mut(panel_entity)
            {
                let delta = trigger.event().delta;
                if let Val::Px(x) = &mut transform.translation.x {
                    *x += delta.x;
                }
                if let Val::Px(y) = &mut transform.translation.y {
                    *y += delta.y;
                }
            }
        })
    }
}

fn prevent_drag_parent(mut trigger: On<Pointer<Drag>>) {
    trigger.propagate(false);
}
