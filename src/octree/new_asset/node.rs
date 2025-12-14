use bevy_camera::primitives::Aabb;
use crate::octree::storage::NodeId;

#[derive(Debug, Clone)]
pub struct OctreeNode<T>
where
    T: Send + Sync,
{
    pub id: NodeId,
    /// child index must follow the following rules:
    /// - Split parent in 2 along all 3 axis. This gives 8 cubes.
    /// - Child index stores child cubes indices along each coordinates in a single number: 0x0XYZ where X, Y and Z are coordinates on corresponding axe
    /// - Child index min value is 0 = 0b000, and max value is 7 = 0b111
    pub child_index: usize,
    pub parent_id: Option<NodeId>,
    pub children: [NodeId; 8],
    pub children_mask: u8,
    pub bounding_box: Aabb,
    pub depth: u32,
    pub data: T,
}
