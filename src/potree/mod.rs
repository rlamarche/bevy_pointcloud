pub mod asset;
mod gizmo;
mod hierarchy;
pub mod loader;
mod point_cloud;
mod points;
pub mod prelude;
mod spawn_async_task;

use crate::potree::gizmo::{init_gizmos, update_gizmos};
use crate::potree::hierarchy::{init_hierarchy_task, update_hierarchy};
use crate::potree::loader::PotreeLoader;
use crate::potree::points::{init_load_points_task, load_points_tx};
use asset::PotreePointCloud;
use bevy_app::{App, Plugin, PreUpdate};
use bevy_asset::AssetApp;
use bevy_ecs::prelude::*;

pub struct PotreePlugin;

impl Plugin for PotreePlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<PotreePointCloud>()
            .register_asset_loader(PotreeLoader {})
            .add_systems(
                PreUpdate,
                (
                    init_hierarchy_task.before(update_hierarchy),
                    init_load_points_task.before(load_points_tx),
                    update_hierarchy,
                    load_points_tx.after(update_hierarchy),
                ),
            )
            .add_systems(PreUpdate, (init_gizmos, update_gizmos.after(init_gizmos)));
    }
}
