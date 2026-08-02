mod node;
mod octree;

use bevy::{asset::Handle, reflect::Reflect};
pub use node::*;
pub use octree::*;

use crate::PointCloudChunk;

#[derive(Debug, Clone, Default, Reflect)]
pub enum PointCloudTopology {
    /// Because we need a default value for [`Reflect`].
    #[default]
    Empty,
    /// A simple, non-streamed point cloud.
    /// Loaded fully into a single mesh, ideal for small assets.
    Flat(Handle<PointCloudChunk>),

    /// A massive, streamed point cloud utilizing an octree structure.
    Octree(OctreeTopology),
}

impl PointCloudTopology {
    pub fn as_octree(&self) -> Option<&OctreeTopology> {
        match self {
            PointCloudTopology::Octree(octree) => Some(octree),
            _ => None,
        }
    }
    pub fn as_octree_mut(&mut self) -> Option<&mut OctreeTopology> {
        match self {
            PointCloudTopology::Octree(octree) => Some(octree),
            _ => None,
        }
    }
    pub fn as_flat(&self) -> Option<&Handle<PointCloudChunk>> {
        match self {
            PointCloudTopology::Flat(chunk_handle) => Some(chunk_handle),
            _ => None,
        }
    }
    pub fn as_flat_mut(&mut self) -> Option<&mut Handle<PointCloudChunk>> {
        match self {
            PointCloudTopology::Flat(chunk_handle) => Some(chunk_handle),
            _ => None,
        }
    }
}
