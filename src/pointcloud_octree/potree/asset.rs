use crate::octree::asset::{NodeId, Octree};
use bevy_math::prelude::*;
use bevy_reflect::TypePath;
use bytemuck::{Pod, Zeroable};

pub type PotreeHierarchyOctree = Octree<PotreeNodeData>;

#[derive(Default, Debug, Clone, TypePath)]
pub struct PotreeNodeData {
    pub spacing: f32,
    pub level: u32,
    pub num_points: usize,
    /// the node id of the loaded points, if loaded
    pub octree_node_id: Option<NodeId>,
}
