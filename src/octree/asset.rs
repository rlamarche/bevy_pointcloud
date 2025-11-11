use crate::octree::storage::GenerationalSlab;
use bevy_asset::Asset;
use bevy_camera::primitives::Aabb;
use bevy_reflect::{Reflect, TypePath};
use std::fmt::Debug;
use thiserror::Error;

pub use super::storage::NodeId;

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
}

#[derive(Debug, Clone, Asset, Reflect)]
pub struct Octree<T>
where
    T: Send + Sync + TypePath,
{
    #[reflect(ignore)]
    pub(crate) nodes: GenerationalSlab<OctreeNode<T>>,
    pub(crate) root_id: Option<NodeId>,
    pub(crate) added: Vec<NodeId>,
    pub(crate) modified: Vec<NodeId>,
    pub(crate) removed: Vec<NodeId>,
}

#[derive(Debug, Clone)]
pub struct OctreeNode<T>
where
    T: Send + Sync,
{
    pub id: NodeId,
    pub child_index: usize,
    pub parent_id: Option<NodeId>,
    pub children: [NodeId; 8],
    pub children_mask: u8,
    pub bounding_box: Aabb,
    pub data: T,
}

impl<T> Octree<T>
where
    T: Send + Sync + TypePath,
{
    pub fn new() -> Self {
        Self {
            nodes: GenerationalSlab::new(),
            root_id: None,
            added: Vec::new(),
            modified: Vec::new(),
            removed: Vec::new(),
        }
    }

    pub fn from_root(bounding_box: Aabb, data: T) -> Self {
        let mut nodes = GenerationalSlab::new();

        let root = OctreeNode {
            id: NodeId::default(),
            child_index: 0,
            parent_id: None,
            children: [NodeId::default(); 8],
            children_mask: 0b00000000,
            bounding_box,
            data,
        };

        // insert the root
        let root_id = nodes.insert(root);

        // store its id
        nodes.get_mut(root_id).unwrap().id = root_id;

        let mut added = Vec::new();
        // tracing root insertion
        added.push(root_id);

        Self {
            nodes,
            root_id: Some(root_id),
            added,
            modified: Vec::new(),
            removed: Vec::new(),
        }
    }

    /// Inserts a new child node to an existing node
    pub fn insert(
        &mut self,
        parent_id_and_child_index: Option<(NodeId, usize)>,
        bounding_box: Aabb,
        data: T,
    ) -> Result<NodeId, InsertNodeError> {
        if let Some((parent_id, child_index)) = &parent_id_and_child_index {
            if *child_index >= 8 {
                return Err(InsertNodeError::ChildIndexOutOfBounds);
            }
            // check the parent
            match self.nodes.get(*parent_id) {
                None => {
                    return Err(InsertNodeError::ParentNotExists);
                }
                Some(parent) => {
                    if (parent.children_mask & (1_u8 << child_index)) > 0 {
                        return Err(InsertNodeError::ChildIndexOccupied);
                    }
                }
            };
        } else {
            if self.root_id.is_some() {
                return Err(InsertNodeError::RootAlreadyExists);
            }
        }

        // insert the new node
        let id = self.nodes.insert(OctreeNode {
            // will set just after
            id: Default::default(),
            parent_id: parent_id_and_child_index.map(|(parent_id, _)| parent_id),
            child_index: parent_id_and_child_index
                .map(|(_, child_index)| child_index)
                .unwrap_or(0),
            children: [NodeId::default(); 8],
            children_mask: 0b00000000,
            bounding_box,
            data,
        });

        // update self id
        self.nodes.get_mut(id).unwrap().id = id;

        if let Some((parent_id, child_index)) = &parent_id_and_child_index {
            // infallible because we checked upper
            let parent = self.nodes.get_mut(*parent_id).unwrap();

            // add to children array and update mask
            parent.children[*child_index] = id;
            parent.children_mask |= 1u8 << child_index;
        } else {
            self.root_id = Some(id);
        }

        // tracing insertion
        self.added.push(id);

        Ok(id)
    }

    pub fn get(&self, node_id: NodeId) -> Option<&OctreeNode<T>> {
        self.nodes.get(node_id)
    }

    pub fn root(&self) -> Option<&OctreeNode<T>> {
        self.get(self.root_id?)
    }
}

fn first_one(mask: u8) -> Option<usize> {
    let inverted_mask = !mask;
    (0..8).find(move |&i| (inverted_mask & (1 << i)) == 0)
}
