mod components;
mod events;
mod helpers;
mod resources;

use bevy::{input_focus::tab_navigation::TabGroup, prelude::*};

pub use components::*;
pub use events::*;
pub use helpers::*;
pub use resources::*;

pub struct MyUiPlugin;

impl Plugin for MyUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiState>()
            .init_resource::<UiSettings>()
            // Messages
            // .add_message::<LoadPointCloudMessage>()
            .add_systems(Startup, settings_root.spawn())
            // .add_systems(
            //     PreUpdate,
            //     (
            //         sync_camera_settings, // write UiSettings → ECS components  // refresh the URL text node
            //     ),
            // )
            .add_observer(
                |event: On<UiHoveringEvent>, mut ui_state: ResMut<UiState>| {
                    ui_state.hovering = event.0;
                },
            )
            .add_observer(
                |event: On<UiDraggingEvent>, mut ui_state: ResMut<UiState>| {
                    ui_state.dragging = event.0;
                },
            );
    }
}


fn scene() -> impl SceneList {
    bsn_list![settings_root()]
}

pub fn settings_root() -> impl Scene {
    bsn! {
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Start,
            justify_content: JustifyContent::Start,
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
        }
        TabGroup
        // ThemeBackgroundColor(tokens::WINDOW_BG),
        Children[
            settings_panel()
        ]
    }
}
