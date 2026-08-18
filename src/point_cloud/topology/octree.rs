use std::{collections::VecDeque, sync::Arc};

use bevy::{asset::Handle, camera::primitives::Aabb, reflect::Reflect};
use slotmap::SlotMap;
use thiserror::Error;

use crate::{
    ChildIndex, ChildrenMask, NodeData, NodeId, PointCloudChunk, PointCloudNode,
    PointCloudNodeStatus,
};

#[derive(Debug, Clone, Reflect, Default)]
pub struct OctreeTopology {
    #[reflect(ignore, clone)]
    pub nodes: SlotMap<NodeId, PointCloudNode>,
    pub root: Option<NodeId>,
}

impl OctreeTopology {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_root(&self) -> Option<&PointCloudNode> {
        self.root.and_then(|node_id| self.get_node(node_id))
    }

    pub fn get_node(&self, node_id: NodeId) -> Option<&PointCloudNode> {
        self.nodes.get(node_id)
    }

    pub fn remove_node(&mut self, node_id: NodeId) -> Option<PointCloudNode> {
        let removed_node = self.nodes.remove(node_id)?;

        if let Some(parent_id) = removed_node.parent_id
            && let Some(parent) = self.nodes.get_mut(parent_id)
        {
            parent.children[removed_node.child_index.index() as usize] = NodeId::null();
            let child_mask: ChildrenMask = removed_node.children_mask;
            parent.children_mask &= !child_mask;
        }

        Some(removed_node)
    }

    pub fn get_node_mut(&mut self, node_id: NodeId) -> Option<&mut PointCloudNode> {
        self.nodes.get_mut(node_id)
    }

    /// Iterates through nodes whose chunk is currently loaded.
    pub fn iter_loaded_chunks(&self) -> OctreeChunksIterator<'_> {
        OctreeChunksIterator::new(self)
    }

    /// Inserts the root node into the octree.
    /// Fails if a root already exists.
    pub fn try_insert_root(
        &mut self,
        status: PointCloudNodeStatus,
        point_count: usize,
        data: NodeData,
        aabb: Option<Aabb>,
        chunk: Option<Handle<PointCloudChunk>>,
    ) -> Result<NodeId, OctreeError> {
        if self.root.is_some() {
            return Err(OctreeError::RootAlreadyExists);
        }

        let name: Arc<str> = Arc::from("r");

        let id = self.nodes.insert_with_key(|id| PointCloudNode {
            id,
            name,
            status,
            child_index: ChildIndex::ROOT,
            parent_id: None,
            children: [NodeId::null(); 8],
            children_mask: ChildrenMask::empty(),
            aabb,
            depth: 0,
            data,
            chunk,
            point_count,
        });

        self.root = Some(id);

        Ok(id)
    }

    /// Inserts a child node into an existing parent.
    pub fn try_insert_child(
        &mut self,
        parent_id: NodeId,
        child_index: ChildIndex,
        status: PointCloudNodeStatus,
        point_count: usize,
        data: NodeData,
        aabb: Option<Aabb>,
        chunk: Option<Handle<PointCloudChunk>>,
    ) -> Result<NodeId, OctreeError> {
        // Check parent
        let (parent_depth, new_name) = {
            let parent = self
                .nodes
                .get(parent_id)
                .ok_or(OctreeError::ParentNotExists)?;

            if parent.children_mask.has_child(child_index) {
                return Err(OctreeError::ChildIndexOccupied);
            }

            let new_name: Arc<str> = Arc::from(format!("{}{}", parent.name, child_index.index()));
            (parent.depth, new_name)
        };

        // Insert the child
        let child_id = self.nodes.insert_with_key(|id| PointCloudNode {
            id,
            name: new_name,
            status,
            child_index,
            parent_id: Some(parent_id),
            children: [NodeId::null(); 8],
            children_mask: ChildrenMask::empty(),
            aabb,
            depth: parent_depth + 1,
            data,
            chunk,
            point_count,
        });

        // Update parent relationships
        // Infallible because we checked existence earlier, but we need a mutable borrow now.
        let parent = self.nodes.get_mut(parent_id).unwrap();
        parent.children[child_index.index() as usize] = child_id;
        parent.children_mask |= child_index.into();

        Ok(child_id)
    }
}

/// An iterator that walks the octree and yields nodes that have loaded chunks.
pub struct OctreeChunksIterator<'a> {
    topology: &'a OctreeTopology,
    queue: VecDeque<NodeId>,
}

impl<'a> OctreeChunksIterator<'a> {
    pub fn new(topology: &'a OctreeTopology) -> Self {
        let mut queue = VecDeque::new();
        if let Some(root_id) = topology.root {
            queue.push_back(root_id);
        }
        Self { topology, queue }
    }
}

impl<'a> Iterator for OctreeChunksIterator<'a> {
    type Item = &'a PointCloudNode;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(current_id) = self.queue.pop_front() {
            if let Some(node) = self.topology.get_node(current_id)
                && node.chunk.is_some()
            {
                for i in node.children_mask.iter_one_bits() {
                    let child_id = node.children[i as usize];
                    if !child_id.is_null() {
                        self.queue.push_back(child_id);
                    }
                }
                return Some(node);
            }
        }
        None
    }
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum OctreeError {
    #[error("parent node does not exist")]
    ParentNotExists,
    #[error("root node already exists")]
    RootAlreadyExists,
    #[error("child index is already occupied on the parent node")]
    ChildIndexOccupied,
    #[error("child index is out of bounds (must be 0..=7)")]
    InvalidChildIndex,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::utils::default;

    #[test]
    fn test_insert_root_ok() {
        let mut topology = OctreeTopology::new();
        let root_id = topology
            .try_insert_root(PointCloudNodeStatus::Loaded, 100, default(), None, None)
            .expect("Unable to insert a root node");

        assert_eq!(topology.root, Some(root_id));
        let root = topology.get_node(root_id).unwrap();
        assert_eq!(root.depth, 0);
        assert_eq!(root.name.as_ref(), "r");
    }

    #[test]
    fn test_insert_children_ok() {
        let mut topology = OctreeTopology::new();
        let root_id = topology
            .try_insert_root(PointCloudNodeStatus::Loaded, 100, default(), None, None)
            .unwrap();

        for child_index in 0..8 {
            let child_id = topology
                .try_insert_child(
                    root_id,
                    ChildIndex::try_from(child_index).unwrap(),
                    PointCloudNodeStatus::Loaded,
                    50,
                    default(),
                    None,
                    None,
                )
                .unwrap();

            let child = topology.get_node(child_id).unwrap();
            assert_eq!(child.depth, 1);
            assert_eq!(child.name.as_ref(), format!("r{}", child_index));
        }

        let root = topology.get_node(root_id).unwrap();
        assert_eq!(root.children_mask.bits(), 0b11111111); // All children occupied
    }

    #[test]
    fn test_err_duplicate_root() {
        let mut topology = OctreeTopology::new();
        topology
            .try_insert_root(PointCloudNodeStatus::Loaded, 100, default(), None, None)
            .unwrap();

        let result =
            topology.try_insert_root(PointCloudNodeStatus::Loaded, 100, default(), None, None);
        assert_eq!(result, Err(OctreeError::RootAlreadyExists));
    }

    #[test]
    fn test_err_duplicate_child() {
        let mut topology = OctreeTopology::new();
        let root_id = topology
            .try_insert_root(PointCloudNodeStatus::Loaded, 100, default(), None, None)
            .unwrap();

        topology
            .try_insert_child(
                root_id,
                ChildIndex::X_0_Y_0_Z_0,
                PointCloudNodeStatus::Loaded,
                50,
                default(),
                None,
                None,
            )
            .unwrap();

        let result = topology.try_insert_child(
            root_id,
            ChildIndex::X_0_Y_0_Z_0,
            PointCloudNodeStatus::Loaded,
            50,
            default(),
            None,
            None,
        );
        assert_eq!(result, Err(OctreeError::ChildIndexOccupied));
    }

    #[test]
    fn test_err_parent_not_exists() {
        let mut topology = OctreeTopology::new();
        // creating a fake NodeId
        let fake_node_id = NodeId::default();

        let result = topology.try_insert_child(
            fake_node_id,
            ChildIndex::X_0_Y_0_Z_0,
            PointCloudNodeStatus::Loaded,
            50,
            default(),
            None,
            None,
        );
        assert_eq!(result, Err(OctreeError::ParentNotExists));
    }
}
