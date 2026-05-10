use bevy_ecs::resource::Resource;

#[derive(Resource, Default)]
pub struct UiState {
    pub hovering: bool,
    pub dragging: bool,
}

/// Shared render / visibility settings mirrored into the UI sliders.
#[derive(Resource)]
pub struct UiSettings {
    pub use_edl: bool,
    pub edl_radius: f32,
    pub edl_strength: f32,
    pub edl_neighbour_count: u32,
    pub min_node_size: f32,
    pub point_budget: usize,
    pub skip_visibility: bool,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            use_edl: true,
            edl_radius: 1.4,
            edl_strength: 0.4,
            edl_neighbour_count: 4,
            min_node_size: 30.0,
            point_budget: 1_000_000,
            skip_visibility: false,
        }
    }
}
