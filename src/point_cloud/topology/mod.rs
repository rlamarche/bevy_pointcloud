mod node;
mod octree;

use bevy::{
    asset::Handle,
    reflect::{Reflect, ReflectDeserialize, ReflectSerialize},
};
pub use node::*;
pub use octree::*;
use serde::{Deserialize, Serialize};

use crate::PointCloudChunk;

#[derive(Debug, Clone, Default, Reflect)]
pub enum PointCloudTopology {
    /// Because we need a default value for [`Reflect`].
    #[default]
    Empty,
    Flat(Handle<PointCloudChunk>),
    Octree(OctreeTopology),
}

impl PointCloudTopology {
    pub fn is_octree(&self) -> bool {
        matches!(self, PointCloudTopology::Octree(_))
    }
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

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, Reflect, Serialize, Deserialize)]
#[reflect(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum PointCloudTopologyKind {
    #[default]
    Empty,
    Flat,
    Octree,
}

impl From<&PointCloudTopology> for PointCloudTopologyKind {
    fn from(value: &PointCloudTopology) -> Self {
        match value {
            PointCloudTopology::Empty => PointCloudTopologyKind::Empty,
            PointCloudTopology::Flat(_) => PointCloudTopologyKind::Flat,
            PointCloudTopology::Octree(_) => PointCloudTopologyKind::Octree,
        }
    }
}
