use crate::octree::storage::GenerationalSlab;
use bevy_asset::Asset;
use bevy_camera::primitives::Aabb;
use bevy_platform::collections::HashSet;
use bevy_reflect::TypePath;
use std::fmt::Debug;
use thiserror::Error;

pub use super::storage::NodeId;

#[derive(Error, Debug)]
pub enum InsertNodeError {
    #[error("parent does not exists")]
    ParentNotExists,
    #[error("root already exists")]
    RootAlreadyExists,
    #[error("parent has already 8 children")]
    ParentChildrenFull,
}

#[derive(Debug, Clone, Asset, TypePath)]
pub struct Octree<T>
where
    T: Clone + Debug + Send + Sync + TypePath,
{
    pub(crate) nodes: GenerationalSlab<OctreeNode<T>>,
    pub(crate) root_id: Option<NodeId>,
    pub(crate) added: HashSet<NodeId>,
    pub(crate) modified: HashSet<NodeId>,
    pub(crate) removed: HashSet<NodeId>,
}

#[derive(Debug, Clone, TypePath)]
pub struct OctreeNode<T>
where
    T: Clone + Debug + Send + Sync + TypePath,
{
    pub id: NodeId,
    pub parent_id: Option<NodeId>,
    pub children: [NodeId; 8],
    pub children_mask: u8,
    pub bounding_box: Aabb,
    pub data: T,
}

impl<T> Default for Octree<T>
where
    T: Clone + Default + Debug + Send + Sync + TypePath,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Octree<T>
where
    T: Clone + Default + Debug + Send + Sync + TypePath,
{
    pub fn new() -> Self {
        Self {
            nodes: GenerationalSlab::new(),
            root_id: None,
            added: HashSet::new(),
            modified: HashSet::new(),
            removed: HashSet::new(),
        }
    }

    pub fn from_root(bounding_box: Aabb, data: T) -> Self {
        let mut nodes = GenerationalSlab::new();

        let root = OctreeNode {
            id: NodeId::default(),
            parent_id: None,
            children: [NodeId::default(); 8],
            children_mask: 0x00,
            bounding_box,
            data,
        };

        // insert the root
        let root_id = nodes.insert(root);

        // store its id
        nodes.get_mut(root_id).unwrap().id = root_id;

        let mut added = HashSet::new();
        // tracing root insertion
        added.insert(root_id);

        Self {
            nodes,
            root_id: Some(root_id),
            added,
            modified: HashSet::new(),
            removed: HashSet::new(),
        }
    }

    pub fn insert(
        &mut self,
        parent_id: Option<NodeId>,
        bounding_box: Aabb,
        data: T,
    ) -> Result<NodeId, InsertNodeError> {
        if let Some(parent_id) = &parent_id {
            // check the parent
            match self.nodes.get(*parent_id) {
                None => {
                    return Err(InsertNodeError::ParentNotExists);
                }
                Some(parent) => {
                    if parent.children_mask == 0xFF {
                        return Err(InsertNodeError::ParentChildrenFull);
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
            id: Default::default(),
            parent_id,
            children: [NodeId::default(); 8],
            children_mask: 0x00,
            bounding_box,
            data,
        });

        // update self id
        self.nodes.get_mut(id).unwrap().id = id;

        if let Some(parent_id) = &parent_id {
            // infallible because we checked upper
            let parent = self.nodes.get_mut(*parent_id).unwrap();

            // get the next free child index
            let child_index = parent.children_mask.trailing_ones() as usize;

            // add to children array and update mask
            parent.children[child_index] = *parent_id;
            parent.children_mask &= !(1 << child_index);
        } else {
            self.root_id = Some(id);
        }

        // tracing insertion
        self.added.insert(id);

        Ok(id)
    }

    pub fn get(&self, node_id: NodeId) -> Option<&OctreeNode<T>> {
        self.nodes.get(node_id)
    }

    pub fn root(&self) -> Option<&OctreeNode<T>> {
        self.get(self.root_id?)
    }
}
