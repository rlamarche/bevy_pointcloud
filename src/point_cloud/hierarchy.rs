use std::{any::Any, sync::Arc};

use bevy::{
    asset::Handle,
    camera::primitives::Aabb,
    prelude::Deref,
    reflect::{std_traits::ReflectDefault, Reflect},
};
use slotmap::{new_key_type, SlotMap};
use thiserror::Error;

use crate::{iter_one_bits, PointCloudChunk};

new_key_type! { pub struct NodeId; }

#[derive(Copy, Clone, Debug, Default, Deref, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChildIndex(u8);

impl TryFrom<u8> for ChildIndex {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0..8 => Ok(ChildIndex(value)),
            _ => Err("Invalid child index"),
        }
    }
}

impl ChildIndex {

    #[inline]
    pub fn is_child(&self) -> bool {
        self.0 < 8
    }

    #[inline]
    pub fn is_root(&self) -> bool {
        self.0 == 8
    }

    #[inline]
    pub fn index(&self) -> usize {
        self.0 as usize
    }

    pub const X_0_Y_0_Z_0: ChildIndex = ChildIndex(0);
    pub const X_0_Y_0_Z_1: ChildIndex = ChildIndex(1);
    pub const X_0_Y_1_Z_0: ChildIndex = ChildIndex(2);
    pub const X_0_Y_1_Z_1: ChildIndex = ChildIndex(3);
    pub const X_1_Y_0_Z_0: ChildIndex = ChildIndex(4);
    pub const X_1_Y_0_Z_1: ChildIndex = ChildIndex(5);
    pub const X_1_Y_1_Z_0: ChildIndex = ChildIndex(6);
    pub const X_1_Y_1_Z_1: ChildIndex = ChildIndex(7);
    pub const ROOT: ChildIndex = ChildIndex(8);
}

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Default, Hash, Clone, Copy, PartialEq, Eq, Debug, Reflect)]
    #[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
    #[reflect(opaque, Default, Hash, Clone, PartialEq, Debug)]
    #[cfg_attr(feature = "serialize", reflect(Serialize, Deserialize))]
    pub struct ChildrenMask: u8 {
        const X_0_Y_0_Z_0 = 1 << 0;
        const X_0_Y_0_Z_1 = 1 << 1;
        const X_0_Y_1_Z_0 = 1 << 2;
        const X_0_Y_1_Z_1 = 1 << 3;
        const X_1_Y_0_Z_0 = 1 << 4;
        const X_1_Y_0_Z_1 = 1 << 5;
        const X_1_Y_1_Z_0 = 1 << 6;
        const X_1_Y_1_Z_1 = 1 << 7;
        const EMPTY = 0;
        const FULL = 0b11111111;
    }
}

impl ChildrenMask {
    pub fn iter_one_bits(&self) -> impl Iterator<Item = u8> {
        iter_one_bits(self.bits())
    }
}

impl From<ChildIndex> for ChildrenMask {
    #[inline]
    fn from(value: ChildIndex) -> Self {
        ChildrenMask::from_bits_retain(1 << *value)
    }
}

#[derive(Clone, Debug)]
pub struct HierarchyNode {
    pub id: NodeId,
    /// The name of the node in the hierarchy with the following format:
    /// - "r" for the root node
    /// - "r" followed by the child index of the ancestors for a child node
    ///
    /// Examples: r, r0, r3, r4, r01, r07, r30, ...
    pub name: Arc<str>,
    pub point_count: usize,
    pub status: HierarchyNodeStatus,
    /// the child index of the current node
    pub child_index: ChildIndex,
    pub parent_id: Option<NodeId>,
    pub children: [NodeId; 8],
    /// the children mask stores, in binary form, the mask of existing children
    pub children_mask: ChildrenMask,
    pub aabb: Option<Aabb>,
    pub depth: u32,
    /// the custom data that the loader may reuse to load children
    pub data: HierarchyData,
    pub chunk: Option<Handle<PointCloudChunk>>,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum HierarchyNodeStatus {
    #[default]
    Proxy,
    Loading,
    Loaded,
}

#[derive(Clone, Debug)]
pub struct InsertHierarchyNode {
    pub status: HierarchyNodeStatus,
    pub child_index: ChildIndex,
    pub aabb: Option<Aabb>,
    pub data: Arc<dyn Any + Send + Sync>,
}

#[derive(Error, Debug, Clone)]
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

#[derive(Clone, Default, Debug, Reflect)]
#[reflect(opaque, Default, Clone, Debug)]
pub struct PointCloudHierarchy {
    pub(crate) nodes: SlotMap<NodeId, HierarchyNode>,
    pub(crate) root: Option<NodeId>,
}

impl PointCloudHierarchy {
    pub fn get_root(&self) -> Option<&HierarchyNode> {
        self.root.and_then(|node_id| self.get_node(node_id))
    }

    /// Inserts a new child node to an existing node
    pub fn insert_hierarchy_node(
        &mut self,
        parent_key: Option<NodeId>,
        child_index: ChildIndex,
        status: HierarchyNodeStatus,
        point_count: usize,
        data: HierarchyData,
        aabb: Option<Aabb>,
        chunk: Option<Handle<PointCloudChunk>>,
    ) -> Result<NodeId, InsertNodeError> {
        let mut depth = None;
        let name;
        if let Some(parent_id) = parent_key {
            if !child_index.is_child() {
                return Err(InsertNodeError::ChildIndexOutOfBounds);
            }
            // check the parent
            match self.nodes.get(parent_id) {
                None => {
                    return Err(InsertNodeError::ParentNotExists);
                }
                Some(parent) => {
                    if (parent.children_mask.bits() & (1_u8 << *child_index)) > 0 {
                        return Err(InsertNodeError::ChildIndexOccupied);
                    }
                    depth = Some(parent.depth + 1);
                    name = Arc::from(format!("{}{}", parent.name, *child_index));
                }
            };
        } else {
            if self.root.is_some() {
                return Err(InsertNodeError::RootAlreadyExists);
            }
            name = Arc::from("r");
        }

        // insert the new node
        let key = self.nodes.insert_with_key(|key| HierarchyNode {
            id: key,
            name,
            status,
            child_index,
            parent_id: parent_key,
            children: [Default::default(); 8],
            children_mask: ChildrenMask::empty(),
            aabb,
            depth: depth.unwrap_or_default(),
            data,
            chunk,
            point_count,
        });

        // update parent children / children_mask
        if let Some(parent_id) = parent_key {
            // infallible because we checked upper
            let parent = self.nodes.get_mut(parent_id).unwrap();

            // add to children array and update mask
            parent.children[*child_index as usize] = key;
            parent.children_mask |= child_index.into();
        } else {
            self.root = Some(key);
        }

        Ok(key)
    }

    pub fn get_node(&self, node_id: NodeId) -> Option<&HierarchyNode> {
        self.nodes.get(node_id)
    }

    pub fn get_node_mut(&mut self, node_id: NodeId) -> Option<&mut HierarchyNode> {
        self.nodes.get_mut(node_id)
    }
}

#[derive(Clone, Debug, Deref)]
pub struct HierarchyData(pub Arc<dyn Any + Send + Sync + 'static>);

impl HierarchyData {
    pub fn new<H>(data: H) -> Self
    where
        H: Send + Sync + 'static,
    {
        Self(Arc::new(data))
    }
}

impl Default for HierarchyData {
    fn default() -> Self {
        Self(Arc::new(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ok() {
        let mut hierarchy = PointCloudHierarchy::default();
        let root_key = hierarchy
            .insert_hierarchy_node(
                None,
                ChildIndex::ROOT,
                HierarchyNodeStatus::Loaded,
                0,
                HierarchyData::default(),
                None,
                None,
            )
            .expect("Unable to insert a root node");

        for child_index in 0..8 {
            hierarchy
                .insert_hierarchy_node(
                    Some(root_key),
                    ChildIndex::try_from(child_index).unwrap(),
                    HierarchyNodeStatus::Loaded,
                    0,
                    HierarchyData::default(),
                    None,
                    None,
                )
                .expect("Unable to insert a child node");
        }
    }

    #[test]
    fn test_ko_duplicate_root() {
        let mut hierarchy = PointCloudHierarchy::default();
        let _ = hierarchy
            .insert_hierarchy_node(
                None,
                ChildIndex::ROOT,
                HierarchyNodeStatus::Loaded,
                0,
                HierarchyData::default(),
                None,
                None,
            )
            .expect("Unable to insert a root node");

        let result = hierarchy.insert_hierarchy_node(
            None,
            ChildIndex::ROOT,
            HierarchyNodeStatus::Loaded,
            0,
            HierarchyData::default(),
            None,
            None,
        );

        assert!(matches!(result, Err(InsertNodeError::RootAlreadyExists)));
    }

    #[test]
    fn test_ko_duplicate_child() {
        let mut hierarchy = PointCloudHierarchy::default();
        let root_key = hierarchy
            .insert_hierarchy_node(
                None,
                ChildIndex::ROOT,
                HierarchyNodeStatus::Loaded,
                0,
                HierarchyData::default(),
                None,
                None,
            )
            .expect("Unable to insert a root node");

        hierarchy
            .insert_hierarchy_node(
                Some(root_key),
                ChildIndex::X_0_Y_0_Z_0,
                HierarchyNodeStatus::Loaded,
                0,
                HierarchyData::default(),
                None,
                None,
            )
            .expect("Unable to insert a child node");

        let result = hierarchy.insert_hierarchy_node(
            Some(root_key),
            ChildIndex::X_0_Y_0_Z_0,
            HierarchyNodeStatus::Loaded,
            0,
            HierarchyData::default(),
            None,
            None,
        );

        assert!(matches!(result, Err(InsertNodeError::ChildIndexOccupied)));
    }
}
