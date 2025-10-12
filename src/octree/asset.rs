use crate::octree::storage::GenerationalSlab;
use bevy_asset::Asset;
use bevy_camera::primitives::Aabb;
use bevy_reflect::TypePath;
use std::fmt::Debug;
use thiserror::Error;

pub use super::storage::NodeId;

#[derive(Error, Debug)]
pub enum InsertNodeError {
    #[error("parent does not exists")]
    ParentNotExists,
    #[error("parent has already 8 children")]
    ParentChildrenFull,
}

#[derive(Debug, Clone, Asset, TypePath)]
pub struct Octree<T>
where
    T: Clone + Default + Debug + Send + Sync + TypePath,
{
    pub nodes: GenerationalSlab<OctreeNode<T>>,
    pub root_id: NodeId,
}

#[derive(Debug, Clone, TypePath)]
pub struct OctreeNode<T>
where
    T: Clone + Default + Debug + Send + Sync + TypePath,
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
        Self::new(Aabb::default(), T::default())
    }
}

impl<T> Octree<T>
where
    T: Clone + Default + Debug + Send + Sync + TypePath,
{
    pub fn new(bounding_box: Aabb, data: T) -> Self {
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

        Self { nodes, root_id }
    }

    pub fn insert(
        &mut self,
        parent_id: NodeId,
        bounding_box: Aabb,
        data: T,
    ) -> Result<NodeId, InsertNodeError> {
        // check the parent
        match self.nodes.get(parent_id) {
            None => {
                return Err(InsertNodeError::ParentNotExists);
            }
            Some(parent) => {
                if parent.children_mask == 0xFF {
                    return Err(InsertNodeError::ParentChildrenFull);
                }
            }
        };

        // insert the new node
        let id = self.nodes.insert(OctreeNode {
            id: Default::default(),
            parent_id: Some(parent_id),
            children: [NodeId::default(); 8],
            children_mask: 0x00,
            bounding_box,
            data,
        });

        // update self id
        self.nodes.get_mut(id).unwrap().id = id;

        // infallible because we checked upper
        let parent = self.nodes.get_mut(parent_id).unwrap();

        // get the next free child index
        let child_index = parent.children_mask.trailing_ones() as usize;

        // add to children array and update mask
        parent.children[child_index] = parent_id;
        parent.children_mask &= !(1 << child_index);

        Ok(id)
    }
}
