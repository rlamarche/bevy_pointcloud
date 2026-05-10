use bevy_asset::Handle;
use bevy_ecs::component::Component;
use bevy_pointcloud::point_cloud_material::PointCloudMaterial;

// ---------------------------------------------------------------------------
// Markers
// ---------------------------------------------------------------------------

/// Marks the root node of our settings panel so we can query it.
#[derive(Component)]
pub struct SettingsPanel;

/// Marks a UI Node to be draggable
#[derive(Component)]
pub struct DraggablePanel;

/// Marks the entity owning the `PointCloudOctree3d` so we can despawn it on reload.
#[derive(Component)]
pub struct PointCloudRoot;

/// Keeps the material handle alive for the lifetime of the app.
#[derive(Component)]
pub struct MyMaterial(#[allow(unused)] pub Handle<PointCloudMaterial>);

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
