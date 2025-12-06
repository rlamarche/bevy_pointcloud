use crate::octree::new_asset::hierarchy::{
    HierarchyNode, HierarchyNodeData, HierarchyNodeStatus, HierarchyOctreeNode,
};
use crate::octree::storage::{GenerationalSlab, NodeId};
use bevy_asset::Asset;
use bevy_camera::primitives::Aabb;
use bevy_platform::collections::HashMap;
use bevy_reflect::TypePath;
use std::fmt::Debug;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum InsertNodeError {
    #[error("parent does not exists")]
    ParentNotExists,
    #[error("root already exists")]
    RootAlreadyExists,
    #[error("child index already occupied")]
    ChildIndexOccupied,
    #[error("child index is out of bounds")]
    ChildIndexOutOfBounds,
    #[error("node not found")]
    NodeNotFound,
}

#[derive(Debug, Clone, TypePath, Asset)]
pub struct NewOctree<H, T>
where
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
{
    pub(crate) hierarchy: GenerationalSlab<HierarchyOctreeNode<H>>,
    pub(crate) nodes: HashMap<NodeId, NewOctreeNode<T>>,
    pub(crate) root_id: Option<NodeId>,

    pub(crate) added: Vec<NodeId>,
    pub(crate) modified: Vec<NodeId>,
    pub(crate) removed: Vec<NodeId>,
}

#[derive(Debug, Clone)]
pub struct NewOctreeNode<T>
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

impl<H, T> NewOctree<H, T>
where
    H: HierarchyNodeData,
    T: Send + Sync + TypePath,
{
    pub fn new() -> Self {
        Self {
            hierarchy: GenerationalSlab::new(),
            nodes: HashMap::new(),
            root_id: None,
            added: Vec::new(),
            modified: Vec::new(),
            removed: Vec::new(),
        }
    }

    pub fn update_hierarchy_node(&mut self, node_id: NodeId, node: HierarchyNode<H>) -> Result<(), InsertNodeError> {
        let Some(hierarchy_octree_node) = self.hierarchy.get_mut(node_id) else {
            return Err(InsertNodeError::NodeNotFound)
        };

        hierarchy_octree_node.status = node.status;
        hierarchy_octree_node.child_index = node.child_index;
        hierarchy_octree_node.bounding_box = node.bounding_box;
        hierarchy_octree_node.data = node.data;

        Ok(())
    }

    /// Inserts a new child node to an existing node
    pub fn insert_hierarchy_node(
        &mut self,
        parent_id: Option<NodeId>,
        node: HierarchyNode<H>,
    ) -> Result<NodeId, InsertNodeError> {
        let mut depth = None;
        if let Some(parent_id) = &parent_id {
            if node.child_index >= 8 {
                return Err(InsertNodeError::ChildIndexOutOfBounds);
            }
            // check the parent
            match self.hierarchy.get(*parent_id) {
                None => {
                    return Err(InsertNodeError::ParentNotExists);
                }
                Some(parent) => {
                    if (parent.children_mask & (1_u8 << node.child_index)) > 0 {
                        return Err(InsertNodeError::ChildIndexOccupied);
                    }
                    depth = Some(parent.depth + 1);
                }
            };
        } else {
            if self.root_id.is_some() {
                return Err(InsertNodeError::RootAlreadyExists);
            }
        }

        // insert the new node
        let vacant_entry = self.hierarchy.vacant_entry();

        let id = vacant_entry.key();

        vacant_entry.insert(HierarchyOctreeNode::<H> {
            id,
            status: node.status,
            child_index: node.child_index,
            parent_id,
            children: [Default::default(); 8],
            children_mask: 0,
            bounding_box: node.bounding_box,
            depth: depth.unwrap_or_default(),
            data: node.data,
        });

        // update parent children / children_mask
        if let Some(parent_id) = &parent_id {
            // infallible because we checked upper
            let parent = self.hierarchy.get_mut(*parent_id).unwrap();

            // add to children array and update mask
            parent.children[node.child_index] = id;
            parent.children_mask |= 1u8 << node.child_index;
        } else {
            self.root_id = Some(id);
        }

        // tracing insertion
        self.added.push(id);

        Ok(id)
    }

    pub fn hierarchy_node(&self, node_id: NodeId) -> Option<&HierarchyOctreeNode<H>> {
        self.hierarchy.get(node_id)
    }

    pub fn hierarchy_node_mut(&mut self, node_id: NodeId) -> Option<&mut HierarchyOctreeNode<H>> {
        self.hierarchy.get_mut(node_id)
    }

    pub fn hierarchy_root(&self) -> Option<&HierarchyOctreeNode<H>> {
        self.root_id.and_then(|root_id| self.hierarchy_node(root_id))
    }
}
