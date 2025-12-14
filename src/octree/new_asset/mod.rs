pub mod asset;

pub mod hierarchy;
pub mod loader;
pub mod node;
pub mod server;
pub mod visibility;

use asset::NewOctree;
use loader::{OctreeLoader, process_octree_load_tasks};
use server::{OctreeServer, handle_internal_octree_events};

use bevy_app::{App, Plugin, PreUpdate};
use bevy_asset::{AssetApp, AssetId, Assets};
use bevy_ecs::prelude::*;
use bevy_reflect::TypePath;
use hierarchy::HierarchyNodeData;
use std::marker::PhantomData;
use visibility::check_octree_nodes_visibility;

pub struct NewOctreeServerPlugin<L, H, T, C, A>(PhantomData<fn() -> (L, H, T, C, A)>);

impl<L, H, T, C, A> Default for NewOctreeServerPlugin<L, H, T, C, A> {
    fn default() -> Self {
        NewOctreeServerPlugin(PhantomData)
    }
}
impl<L, H, T, C, A> Plugin for NewOctreeServerPlugin<L, H, T, C, A>
where
    L: OctreeLoader<H, T> + 'static,
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
    C: Component,
    for<'a> &'a C: Into<AssetId<NewOctree<H, T>>>,
    A: Send + Sync + 'static,
{
    fn build(&self, app: &mut App) {
        // register asset type if needed
        // TODO do this in a specific plugin
        if !app.world().contains_resource::<Assets<NewOctree<H, T>>>() {
            app.init_asset::<NewOctree<H, T>>();
        }

        app.init_asset::<NewOctree<H, T>>()
            .init_resource::<OctreeServer<L, H, T>>()
            .add_systems(
                PreUpdate,
                (
                    handle_internal_octree_events::<L, H, T>,
                    process_octree_load_tasks::<L, H, T>
                        .after(check_octree_nodes_visibility::<L, H, T, C>),
                ),
            );
    }
}
