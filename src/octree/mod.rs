use crate::octree::asset::Octree;
use bevy_app::{App, Plugin};
use bevy_asset::AssetApp;
use bevy_reflect::{FromReflect, Reflect, TypePath};
use std::fmt::Debug;
use std::marker::PhantomData;

pub mod asset;
pub mod render_asset;
mod storage;

pub mod visibility;

pub struct OctreeAssetPlugin<T>(PhantomData<T>)
where
    T: Clone + Debug + Send + Sync + TypePath;

impl<T> Default for OctreeAssetPlugin<T> where T: Clone + Default + Debug + Send + Sync + TypePath {
    fn default() -> Self {
        OctreeAssetPlugin(PhantomData)
    }
}

impl<T> Plugin for OctreeAssetPlugin<T>
where
    T: Clone + Default + Debug + Send + Sync + TypePath,
{
    fn build(&self, app: &mut App) {
        app
            .init_asset::<Octree<T>>()
            .register_asset_reflect::<Octree<T>>();
    }
}
