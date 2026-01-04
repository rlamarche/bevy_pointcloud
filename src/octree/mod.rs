pub mod asset;

pub mod hierarchy;
pub mod loader;
pub mod node;
pub mod server;
pub mod visibility;
pub mod extract;
pub mod storage;

use asset::Octree;

use bevy_app::{App, Plugin};
use bevy_asset::AssetApp;
use hierarchy::HierarchyNodeData;
use node::NodeData;
use std::marker::PhantomData;
use bevy_ecs::prelude::Component;
use bevy_reflect::TypePath;

pub struct OctreeAssetPlugin<T>(PhantomData<fn() -> T>);

impl<T> Default for OctreeAssetPlugin<T> {
    fn default() -> Self {
        OctreeAssetPlugin(PhantomData)
    }
}
impl<T: NodeData> Plugin for OctreeAssetPlugin<T>
{
    fn build(&self, app: &mut App) {
        app.init_asset::<Octree<T>>();
    }
}
