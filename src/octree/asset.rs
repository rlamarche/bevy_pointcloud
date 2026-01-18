use super::hierarchy::{HierarchyNode, HierarchyNodeStatus, HierarchyOctreeNode};
use super::node::{NodeData, NodeStatus, OctreeNode};
use crate::octree::storage::{GenerationalSlab, NodeId};
use crate::octree::visibility::iter_one_bits;
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

#[derive(Error, Debug)]
pub enum UpdateNodeError {
    #[error("node not found")]
    NodeNotFound,
}

#[derive(TypePath, Asset)]
pub struct Octree<T: NodeData> {
    pub(crate) hierarchy: GenerationalSlab<OctreeNode<T>>,
    pub(crate) root_id: Option<NodeId>,

    /// Contains just added nodes
    pub(crate) added_nodes: Vec<NodeId>,
    /// Contains nodes whose data has just been added
    pub(crate) added_nodes_data: Vec<NodeId>,
    pub(crate) modified_nodes: Vec<NodeId>,
    /// Contains removed nodes
    pub(crate) removed_nodes: Vec<NodeId>,
    /// Contains nodes whose data has just been removed
    pub(crate) removed_nodes_data: Vec<NodeId>,
}

impl<T: NodeData> Octree<T> {
    pub fn new() -> Self {
        Self {
            hierarchy: GenerationalSlab::new(),
            root_id: None,
            added_nodes: Vec::new(),
            added_nodes_data: Vec::new(),
            modified_nodes: Vec::new(),
            removed_nodes: Vec::new(),
            removed_nodes_data: Vec::new(),
        }
    }

    pub(crate) fn clear_tracking(&mut self) {
        self.added_nodes.clear();
        self.added_nodes_data.clear();
        self.modified_nodes.clear();
        self.removed_nodes.clear();
        self.removed_nodes_data.clear();
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
                    name = Arc::from(format!(
                        "{}{}",
                        parent.hierarchy.name, hierarchy_node.child_index
                    ));
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
        self.added_nodes.push(id);

        Ok(id)
    }

    pub fn hierarchy_node(&self, node_id: NodeId) -> Option<&HierarchyOctreeNode> {
        Some(&self.hierarchy.get(node_id)?.hierarchy)
    }

    pub fn hierarchy_root(&self) -> Option<&HierarchyOctreeNode> {
        self.root_id
            .and_then(|root_id| self.hierarchy_node(root_id))
    }

    pub fn node(&self, node_id: NodeId) -> Option<&OctreeNode<T>> {
        self.hierarchy.get(node_id)
    }

    pub fn node_root(&self) -> Option<&OctreeNode<T>> {
        self.root_id.and_then(|root_id| self.node(root_id))
    }

    pub fn root_id(&self) -> Option<NodeId> {
        self.root_id
    }

    pub fn insert_node_data(&mut self, node_id: NodeId, data: T) -> Result<(), UpdateNodeError> {
        let node = self.hierarchy.get_mut(node_id).ok_or(UpdateNodeError::NodeNotFound)?;

        node.data = Some(data);
        node.status = NodeStatus::Loaded;

        self.added_nodes_data.push(node_id);

        Ok(())
    }

    pub fn set_node_hierarchy_loading(&mut self, node_id: NodeId) -> Result<(), UpdateNodeError> {
        let node = self.hierarchy.get_mut(node_id).ok_or(UpdateNodeError::NodeNotFound)?;
        node.hierarchy.status = HierarchyNodeStatus::Loading;

        Ok(())
    }


    pub fn set_node_data_loading(&mut self, node_id: NodeId) -> Result<(), UpdateNodeError> {
        let node = self.hierarchy.get_mut(node_id).ok_or(UpdateNodeError::NodeNotFound)?;
        node.status = NodeStatus::Loading;

        Ok(())
    }

    /// Removes a node from the octree, and all its children, and update parent data
    /// Warn: it does not yet take into account of the "proxy" status of hierarchy
    /// So a removed node will never become a proxy, and will not be loaded again
    /// Prefer [`Self::remove_node_data`] to free memory
    pub fn remove_node(&mut self, node_id: NodeId) -> Option<OctreeNode<T>> {
        // remove the node and its children
        let node = self.remove_nodes_recursively(node_id)?;

        // update parent children mask
        let parent = self.hierarchy.get_mut(node.hierarchy.parent_id?)?;
        parent.hierarchy.children_mask &= !(1u8 << node.hierarchy.child_index);

        Some(node)
    }

    fn remove_nodes_recursively(&mut self, node_id: NodeId) -> Option<OctreeNode<T>> {
        let node = self.hierarchy.remove(node_id)?;
        self.removed_nodes.push(node_id);

        for i in iter_one_bits(node.hierarchy.children_mask) {
            let child_id = node.hierarchy.children[i as usize];
            self.remove_nodes_recursively(child_id);
        }

        Some(node)
    }

    /// Removes data of a node
    pub fn remove_node_data(&mut self, node_id: NodeId) -> Option<T> {
        let node = self.hierarchy.get_mut(node_id)?;

        // remove the data
        let data = node.data.take();

        // set valid node status
        node.status = NodeStatus::HierarchyOnly;

        // trace the remove
        self.removed_nodes_data.push(node_id);

        data
    }
}
