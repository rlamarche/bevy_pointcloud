mod hierarchy;

use bevy::{
    asset::{Asset, AssetId, Handle}, camera::primitives::Aabb, ecs::reflect::ReflectFromWorld, mesh::Mesh, reflect::{Reflect, std_traits::ReflectDefault},
};

pub use hierarchy::*;

#[derive(Asset, Debug, Clone, Default, Reflect)]
#[reflect(Debug, FromWorld, Clone, Default)]
pub struct PointCloud {
    pub(crate) hierarchy: PointCloudHierarchy,
    pub aabb: Option<Aabb>,
}

impl PointCloud {
    pub fn new() -> Self {
        Self::default()
    }
}

/// A chunk of points
#[derive(Asset, Debug, Clone, Default, Reflect)]
#[reflect(Debug, FromWorld, Clone, Default)]
pub struct PointCloudChunk {
    pub mesh_handle: Option<Handle<Mesh>>,
    pub aabb: Option<Aabb>,
    pub vertex_buffer_size: usize,
}


#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct PointCloudNodeKey {
    pub id: AssetId<PointCloud>,
    pub node_id: NodeId,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct PointCloudChunkKey {
    pub id: AssetId<PointCloud>,
    pub chunk_id: AssetId<PointCloudChunk>,
}
