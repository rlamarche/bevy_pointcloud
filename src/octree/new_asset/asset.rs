use super::hierarchy::{HierarchyNode, HierarchyNodeData, HierarchyOctreeNode};
use super::node::{NodeData, NodeStatus, OctreeNode};
use crate::octree::storage::{GenerationalSlab, NodeId};
use bevy_asset::Asset;
use bevy_reflect::TypePath;
use std::fmt::Debug;
use std::sync::Arc;
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

#[derive(TypePath, Asset)]
pub struct NewOctree<T: NodeData>
{
    pub(crate) hierarchy: GenerationalSlab<OctreeNode<T>>,
    pub(crate) root_id: Option<NodeId>,

    pub(crate) added: Vec<NodeId>,
    pub(crate) modified: Vec<NodeId>,
    pub(crate) removed: Vec<NodeId>,
}

impl<T: NodeData> NewOctree<T>
{
    pub fn new() -> Self {
        Self {
            hierarchy: GenerationalSlab::new(),
            root_id: None,
            added: Vec::new(),
            modified: Vec::new(),
            removed: Vec::new(),
        }
    }

    pub fn update_hierarchy_node(
        &mut self,
        node_id: NodeId,
        hierarchy_node: HierarchyNode,
    ) -> Result<(), InsertNodeError> {
        let Some(node) = self.hierarchy.get_mut(node_id) else {
            return Err(InsertNodeError::NodeNotFound);
        };

        node.hierarchy.status = hierarchy_node.status;
        node.hierarchy.child_index = hierarchy_node.child_index;
        node.hierarchy.bounding_box = hierarchy_node.bounding_box;
        node.hierarchy.data = hierarchy_node.data;

        Ok(())
    }

    /// Inserts a new child node to an existing node
    pub fn insert_hierarchy_node(
        &mut self,
        parent_id: Option<NodeId>,
        hierarchy_node: HierarchyNode,
    ) -> Result<NodeId, InsertNodeError> {
        let mut depth = None;
        let name;
        if let Some(parent_id) = &parent_id {
            if hierarchy_node.child_index >= 8 {
                return Err(InsertNodeError::ChildIndexOutOfBounds);
            }
            // check the parent
            match self.hierarchy.get(*parent_id) {
                None => {
                    return Err(InsertNodeError::ParentNotExists);
                }
                Some(parent) => {
                    if (parent.hierarchy.children_mask & (1_u8 << hierarchy_node.child_index)) > 0 {
                        return Err(InsertNodeError::ChildIndexOccupied);
                    }
                    depth = Some(parent.hierarchy.depth + 1);
                    name = Arc::from(format!("{}{}", parent.hierarchy.name, hierarchy_node.child_index));
                }
            };
        } else {
            if self.root_id.is_some() {
                return Err(InsertNodeError::RootAlreadyExists);
            }
            name = Arc::from("r");
        }

        // insert the new node
        let vacant_entry = self.hierarchy.vacant_entry();

        let id = vacant_entry.key();

        vacant_entry.insert(OctreeNode::<T> {
            hierarchy: HierarchyOctreeNode {
                id,
                name,
                status: hierarchy_node.status,
                child_index: hierarchy_node.child_index,
                parent_id,
                children: [Default::default(); 8],
                children_mask: 0,
                bounding_box: hierarchy_node.bounding_box,
                depth: depth.unwrap_or_default(),
                data: hierarchy_node.data,
            },
            status: NodeStatus::HierarchyOnly,
            data: None,
        });

        // update parent children / children_mask
        if let Some(parent_id) = &parent_id {
            // infallible because we checked upper
            let parent = self.hierarchy.get_mut(*parent_id).unwrap();

            // add to children array and update mask
            parent.hierarchy.children[hierarchy_node.child_index as usize] = id;
            parent.hierarchy.children_mask |= 1u8 << hierarchy_node.child_index;
        } else {
            self.root_id = Some(id);
        }

        // tracing insertion
        self.added.push(id);

        Ok(id)
    }

    pub fn hierarchy_node(&self, node_id: NodeId) -> Option<&HierarchyOctreeNode> {
        Some(&self.hierarchy.get(node_id)?.hierarchy)
    }

    pub fn hierarchy_node_mut(&mut self, node_id: NodeId) -> Option<&mut HierarchyOctreeNode> {
        Some(&mut self.hierarchy.get_mut(node_id)?.hierarchy)
    }

    pub fn hierarchy_root(&self) -> Option<&HierarchyOctreeNode> {
        self.root_id
            .and_then(|root_id| self.hierarchy_node(root_id))
    }

    pub fn node(&self, node_id: NodeId) -> Option<&OctreeNode<T>> {
        self.hierarchy.get(node_id)
    }

    pub fn node_mut(&mut self, node_id: NodeId) -> Option<&mut OctreeNode<T>> {
        self.hierarchy.get_mut(node_id)
    }

    pub fn node_root(&self) -> Option<&OctreeNode<T>> {
        self.root_id.and_then(|root_id| self.node(root_id))
    }
}
