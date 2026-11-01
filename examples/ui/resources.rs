use bevy::prelude::*;

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
    pub light_min_node_size: f32,
    pub light_point_budget: usize,
    pub plane_distance_0: f32,
    pub plane_distance_1: f32,
    pub skip_visibility: bool,
    pub xray: bool,
    pub edl_enabled: bool,
    pub weighted_splats: bool,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            use_edl: true,
            edl_radius: 1.4,
            edl_strength: 0.4,
            edl_neighbour_count: 4,
            min_node_size: 30.0,
            point_budget: 10_000_000,
            light_min_node_size: 30.0,
            light_point_budget: 1_000_000,
            plane_distance_0: 20.0,
            plane_distance_1: 20.0,
            skip_visibility: false,
            xray: false,
            edl_enabled: false,
            weighted_splats: false,
        }
    }
}
