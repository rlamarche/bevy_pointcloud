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

pub struct NewOctreeAssetPlugin<H, T>(PhantomData<fn() -> (H, T)>);

impl<H, T> Default for NewOctreeAssetPlugin<H, T> {
    fn default() -> Self {
        NewOctreeAssetPlugin(PhantomData)
    }
}
impl<H, T> Plugin for NewOctreeAssetPlugin<H, T>
where
    H: HierarchyNodeData,
    T: NodeData,
{
    fn build(&self, app: &mut App) {
        app.init_asset::<NewOctree<H, T>>();
    }
}
