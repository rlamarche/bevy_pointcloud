pub mod asset;

pub mod hierarchy;
pub mod loader;
pub mod server;
pub mod visibility;

use asset::NewOctree;
use loader::OctreeLoader;
use server::OctreeServer;

use crate::octree::new_asset::hierarchy::HierarchyNodeData;
use crate::octree::new_asset::server::handle_internal_octree_events;
use bevy_app::{App, Plugin, PreUpdate};
use bevy_asset::{AssetApp, Assets};
use bevy_reflect::TypePath;
use std::marker::PhantomData;

pub struct NewOctreeServerPlugin<L, H, T, C, A>(PhantomData<fn() -> (L, H, T, C, A)>);

impl<L, H, T, C, A> Default for NewOctreeServerPlugin<L, H, T, C, A> {
    fn default() -> Self {
        NewOctreeServerPlugin(PhantomData)
    }
}
impl<L, H, T, C, A> Plugin for NewOctreeServerPlugin<L, H, T, C, A>
where
    L: OctreeLoader<H> + 'static,
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
    C: Send + Sync + 'static,
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
            .add_systems(PreUpdate, handle_internal_octree_events::<L, H, T>);
    }
}
