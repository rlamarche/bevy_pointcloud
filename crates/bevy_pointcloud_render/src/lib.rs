#![expect(missing_docs, reason = "Not all docs are written yet.")]

use bevy_app::{App, Plugin};
use bevy_asset::AssetApp;

extern crate alloc;
extern crate core;

/// Adds [`Mesh`] as an asset.
#[derive(Default)]
pub struct PointCloudRenderPlugin;

impl Plugin for PointCloudRenderPlugin {
    fn build(&self, app: &mut App) {

    }
}
