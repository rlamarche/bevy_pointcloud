use bevy::prelude::*;
use bevy_pointcloud::prelude::*;

// ---------------------------------------------------------------------------
// Markers
// ---------------------------------------------------------------------------

/// Marks the root node of our settings panel so we can query it.
#[derive(Component, Clone, Default)]
pub struct SettingsPanel;

/// Marks a UI Node to be draggable
#[derive(Component, Clone, Default)]
pub struct DraggablePanel;

/// Marks the entity owning the `PointCloudOctree3d` so we can despawn it on reload.
#[derive(Component, Clone, Default)]
pub struct PointCloudRoot;

/// Keeps the material handle alive for the lifetime of the app.
#[derive(Component, Clone)]
pub struct MyMaterial(Handle<SimplePointCloudMaterial>);

// ---------------------------------------------------------------------------
// Button / interaction handling
// ---------------------------------------------------------------------------

// /// Discriminator for toggle rows — avoids one system per setting.
// #[derive(Component, Clone, Copy)]
// pub enum UiSettingToggle {
//     UseEdl,
//     SkipVisibility,
// }

// /// Discriminator for slider rows.
// #[derive(Component, Clone, Copy)]
// pub enum UiSettingSlider {
//     EdlRadius,
//     EdlStrength,
//     MinNodeSize,
//     PointBudget,
// }

// /// Actions attached to buttons.
// #[derive(Component, Clone, Copy)]
// pub enum UiAction {
//     ClearUrl,
//     LoadUrl,
// }
