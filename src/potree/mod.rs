pub mod asset;
mod gizmo;
mod hierarchy;
pub mod loader;
mod point_cloud;
mod points;
pub mod prelude;
mod spawn_async_task;

use crate::potree::gizmo::{init_gizmos, init_text_gizmos, update_gizmos, update_text_gizmos};
use crate::potree::hierarchy::{init_hierarchy_task, update_hierarchy};
use crate::potree::loader::PotreeLoader;
use crate::potree::points::{init_load_points_task, load_points_rx, load_points_tx};
use asset::PotreePointCloud;
use bevy_app::{App, Plugin, PreUpdate, Startup, Update};
use bevy_asset::AssetApp;
use bevy_ecs::prelude::*;
use bevy_rich_text3d::Text3dPlugin;

pub struct PotreePlugin;

impl Plugin for PotreePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Text3dPlugin {
            load_system_fonts: true,
            ..Default::default()
        });

        app.init_asset::<PotreePointCloud>()
            .register_asset_loader(PotreeLoader {})
            .add_systems(Startup, init_text_gizmos)
            .add_systems(
                PreUpdate,
                (
                    init_hierarchy_task.before(update_hierarchy),
                    init_load_points_task.before(load_points_tx),
                    update_hierarchy,
                    load_points_tx.after(update_hierarchy),
                    load_points_rx.after(load_points_tx),
                ),
            )
            .add_systems(PreUpdate, (init_gizmos, update_gizmos.after(init_gizmos)))
            .add_systems(Update, update_text_gizmos);
    }
}
