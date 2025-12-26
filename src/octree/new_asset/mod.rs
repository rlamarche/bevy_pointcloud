pub mod asset;

pub mod hierarchy;
pub mod loader;
pub mod node;
pub mod server;
pub mod visibility;
pub mod extract;

use asset::NewOctree;

use bevy_app::{App, Plugin};
use bevy_asset::{AssetApp, AssetId};
use hierarchy::HierarchyNodeData;
use node::NodeData;
use std::marker::PhantomData;
use bevy_ecs::prelude::Component;
use bevy_reflect::TypePath;

pub struct NewOctreeAssetPlugin<T>(PhantomData<fn() -> T>);

impl<T> Default for NewOctreeAssetPlugin<T> {
    fn default() -> Self {
        NewOctreeAssetPlugin(PhantomData)
    }
}
impl<T: NodeData> Plugin for NewOctreeAssetPlugin<T>
{
    fn build(&self, app: &mut App) {
        app.init_asset::<NewOctree<T>>();
    }
}
