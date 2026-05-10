mod components;
mod events;
mod helpers;
mod messages;
mod resources;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_input_focus::tab_navigation::TabGroup;
use bevy_pointcloud::{
    octree::visibility::components::SkipOctreeVisibility,
    pointcloud_octree::PointCloudOctreeVisibilitySettings, render::PointCloudRenderMode,
};
use bevy_ui::prelude::*;
use bevy_utils::default;
pub use components::*;
pub use events::*;
use helpers::*;
pub use messages::*;
pub use resources::*;

pub struct MyUiPlugin;

impl Plugin for MyUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiState>()
            .init_resource::<UiSettings>()
            // Messages
            .add_message::<LoadPointCloudMessage>()
            .add_systems(Startup, spawn_ui)
            .add_systems(
                PreUpdate,
                (
                    sync_camera_settings, // write UiSettings → ECS components  // refresh the URL text node
                ),
            )
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

/// Spawn the entire settings panel using native Bevy UI nodes.
///
/// Layout: a fixed-width column panel pinned to the top-left corner,
/// containing labelled rows for each setting and a URL input section at the bottom.
fn spawn_ui(mut commands: Commands) {
    commands.spawn(settings_root());
}

pub fn settings_root() -> impl Bundle {
    (
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Start,
            justify_content: JustifyContent::Start,
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            ..default()
        },
        TabGroup::default(),
        // ThemeBackgroundColor(tokens::WINDOW_BG),
        children![settings_panel()],
    )
}

// ---------------------------------------------------------------------------
// Update systems
// ---------------------------------------------------------------------------

/// Write `UiSettings` values back into the camera's ECS components every frame.
///
/// This is cheaper than event-based syncing for continuously-adjusted sliders.
fn sync_camera_settings(
    settings: Res<UiSettings>,
    mut cameras: Query<(
        Entity,
        &mut PointCloudOctreeVisibilitySettings,
        &mut PointCloudRenderMode,
        Option<&SkipOctreeVisibility>,
    )>,
    mut commands: Commands,
) {
    let Ok((entity, mut visibility, mut render_mode, skip)) = cameras.single_mut() else {
        return;
    };

    render_mode.use_edl = settings.use_edl;
    render_mode.edl_radius = settings.edl_radius;
    render_mode.edl_strength = settings.edl_strength;
    render_mode.edl_neighbour_count = settings.edl_neighbour_count;
    visibility.filter = Some(settings.min_node_size);
    visibility.budget = Some(settings.point_budget);

    // Sync the SkipOctreeVisibility marker component.
    match (settings.skip_visibility, skip.is_some()) {
        (true, false) => {
            commands.entity(entity).insert(SkipOctreeVisibility);
        }
        (false, true) => {
            commands.entity(entity).remove::<SkipOctreeVisibility>();
        }
        _ => {}
    }
}
