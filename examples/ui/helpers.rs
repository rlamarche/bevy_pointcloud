use std::collections::VecDeque;

use bevy_ecs::prelude::*;
use bevy_feathers::{
    controls::{button, checkbox, slider, ButtonProps, SliderProps},
    theme::{ThemeBackgroundColor, ThemedText},
    tokens,
};
use bevy_input_focus::tab_navigation::TabIndex;
use bevy_log::info;
use bevy_picking::prelude::*;
use bevy_text::{prelude::*, LineHeight};
use bevy_ui::{prelude::*, Checked};
use bevy_ui_text_input::{TextInputBuffer, TextInputMode, TextInputNode, TextInputPrompt};
use bevy_ui_widgets::{
    checkbox_self_update, observe, slider_self_update, Activate, SliderPrecision, SliderStep,
    ValueChange,
};
use bevy_utils::prelude::*;

use crate::ui::{
    DraggablePanel, LoadPointCloudMessage, SettingsPanel, UiDraggingEvent, UiHoveringEvent,
    UiSettings,
};

// ---------------------------------------------------------------------------
// UI Components
// ---------------------------------------------------------------------------

pub fn settings_panel() -> impl Bundle {
    (
        SettingsPanel,
        DraggablePanel,
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
            ..default()
        },
        ThemeBackgroundColor(tokens::WINDOW_BG),
        ui_drag_observer(),
        ui_hover_observer(),
        children![
            settings_title(),
            settings_checkbox(),
            settings_section("URL", settings_section_url()),
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
                settings_slider(30.0, 1000.0, 30.0, 10.0, 0, |ui_settings, value| {
                    ui_settings.min_node_size = value;
                })
            ),
            settings_section(
                "Point Budget",
                settings_slider(
                    100_000.0,
                    10_000_000.0,
                    1_000_000.0,
                    100.0,
                    -2,
                    |ui_settings, value| {
                        ui_settings.point_budget = value as usize;
                    }
                )
            ),
        ],
    )
}

fn settings_checkbox() -> impl Bundle {
    (
        checkbox(Checked, Spawn((Text::new("Checkbox"), ThemedText))),
        observe(|_change: On<ValueChange<bool>>, mut _commands: Commands| {
            info!("Checkbox clicked!");
        }),
        observe(checkbox_self_update),
        observe(prevent_drag_parent),
    )
}

/// URL Section bundle
fn settings_section_url() -> impl Bundle {
    (
        Node {
            width: percent(100.0),
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: px(5),
            align_items: AlignItems::Center,
            // align_content: AlignContent::Center,
            ..Default::default()
        },
        // children![input_entity, load_button()],
        Children::spawn(SpawnWith(move |parent: &mut ChildSpawner| {
            let input_entity = parent.spawn(url_input()).id();
            parent.spawn(load_url_button(input_entity));
        })),
    )
}

pub fn url_input() -> impl Bundle {
    (
        Node {
            width: percent(100.0),
            height: px(12),
            ..default()
        },
        TabIndex::default(),
        TextInputNode {
            mode: TextInputMode::SingleLine,
            clear_on_submit: false,
            ..Default::default()
        },
        TextInputPrompt {
            text: "Enter URL here...".to_owned(),
            ..Default::default()
        },
        TextFont {
            font_size: 12.,
            ..Default::default()
        },
        LineHeight::RelativeToFont(1.0),
    )
}

fn load_url_button(url_entity: Entity) -> impl Bundle {
    (
        button(
            ButtonProps::default(),
            (),
            Spawn((Text::new("Open"), ThemedText)),
        ),
        observe(
            move |_: On<Activate>, mut commands: Commands, text_buffer: Query<&TextInputBuffer>| {
                if let Ok(text_buffer) = text_buffer.get(url_entity) {
                    let text = text_buffer.get_text();
                    let text = text.trim();
                    if !text.is_empty() {
                        commands.write_message(LoadPointCloudMessage {
                            url: text_buffer.get_text(),
                        });
                    }
                }
            },
        ),
    )
}

fn settings_slider(
    min: f32,
    max: f32,
    value: f32,
    step: f32,
    precision: i32,
    callback: fn(settings: &mut UiSettings, value: f32) -> (),
) -> impl Bundle {
    (
        slider(
            SliderProps { min, max, value },
            (SliderStep(step), SliderPrecision(precision)),
        ),
        observe(
            move |change: On<ValueChange<f32>>, mut ui_settings: ResMut<UiSettings>| {
                callback(&mut ui_settings, change.value);
            },
        ),
        observe(slider_self_update),
        ui_drag_observer(),
    )
}

// ---------------------------------------------------------------------------
// UI Helpers
// ---------------------------------------------------------------------------

fn settings_title() -> impl Bundle {
    (
        Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center, // horizontal
            align_items: AlignItems::Center,         // vertical

            ..default()
        },
        children![(Text("Settings".to_owned()))],
        drag_handle(),
    )
}

fn settings_section(title: &str, content: impl Bundle) -> impl Bundle {
    (
        Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        children![settings_section_title(title), content,],
    )
}

fn settings_section_title(title: &str) -> impl Bundle {
    (
        Text(title.to_owned()),
        TextFont {
            font_size: 12.,
            ..Default::default()
        },
    )
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

fn ui_hover_observer() -> impl Bundle {
    (
        observe(|_: On<Pointer<Over>>, mut commands: Commands| {
            commands.trigger(UiHoveringEvent(true));
        }),
        observe(|_: On<Pointer<Out>>, mut commands: Commands| {
            commands.trigger(UiHoveringEvent(false));
        }),
    )
}

/// Sends dragging event state to ui state to prevent camera movements while dragging
fn ui_drag_observer() -> impl Bundle {
    (
        observe(|_: On<Pointer<DragStart>>, mut commands: Commands| {
            commands.trigger(UiDraggingEvent(true));
        }),
        observe(|_: On<Pointer<DragEnd>>, mut commands: Commands| {
            commands.trigger(UiDraggingEvent(false));
        }),
    )
}

/// Returns an ancestor having a specific component
fn find_ancestor<C: Component>(
    mut entity: Entity,
    parent_query: Query<(&ChildOf, Has<C>)>,
) -> Option<Entity> {
    while let Ok((ChildOf(parent), has_marker)) = parent_query.get(entity) {
        if has_marker {
            return Some(entity);
        } else {
            entity = *parent;
        }
    }

    None
}

/// Returns the first descendant found having a specific component
#[allow(unused)]
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
            } else {
                if let Some(children) = children {
                    for child in children {
                        stack.push_back(*child);
                    }
                }
            }
        }
    }

    None
}

fn drag_handle() -> impl Bundle {
    observe(
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
        },
    )
}

fn prevent_drag_parent(mut trigger: On<Pointer<Drag>>) {
    trigger.propagate(false);
}
