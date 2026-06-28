#![expect(missing_docs, reason = "Not all docs are written yet.")]

use bevy_app::{App, Plugin};

extern crate alloc;
extern crate core;

mod pointcloud;

/// Adds [`Mesh`] as an asset.
#[derive(Default)]
pub struct MeshPlugin;

impl Plugin for MeshPlugin {
    fn build(&self, app: &mut App) {
        // app.init_asset::<PointCloud>()
        //     .register_asset_reflect::<PointCloud>();
    }
}
