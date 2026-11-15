mod topology;

use bevy::{
    asset::{Asset, AssetId, Handle},
    camera::primitives::Aabb,
    ecs::reflect::ReflectFromWorld,
    mesh::Mesh,
    reflect::{std_traits::ReflectDefault, Reflect},
};

pub use topology::*;

#[derive(Asset, Debug, Clone, Default, Reflect)]
#[reflect(Debug, FromWorld, Clone, Default)]
pub struct PointCloud {
    #[reflect(ignore, clone)]
    pub topology: PointCloudTopology,
    pub spacing: Option<f32>,
    pub aabb: Option<Aabb>,
}

impl PointCloud {
    pub fn new_octree() -> Self {
        Self {
            topology: PointCloudTopology::Octree(OctreeTopology::new()),
            ..Default::default()
        }
    }
}

/// A chunk of points
#[derive(Asset, Debug, Clone, Default, Reflect)]
#[reflect(Debug, FromWorld, Clone, Default)]
pub struct PointCloudChunk {
    pub topology: PointCloudChunkTopology,
    pub point_count: usize,
    pub mesh_handle: Option<Handle<Mesh>>,
    pub aabb: Option<Aabb>,
    pub vertex_buffer_size: usize,
}

impl PointCloudChunk {
    pub fn depth(&self) -> u32 {
        match &self.topology {
            PointCloudChunkTopology::Empty | PointCloudChunkTopology::Flat => 0,
            PointCloudChunkTopology::OctreeNode(node) => node.depth,
        }
    }
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
