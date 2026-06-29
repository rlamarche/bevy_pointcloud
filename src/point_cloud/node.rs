use std::{any::Any, sync::Arc};

use bevy::{
    asset::Handle,
    camera::primitives::Aabb,
    prelude::Deref,
    reflect::{std_traits::ReflectDefault, Reflect},
};
use slotmap::{new_key_type, Key, KeyData};

use crate::PointCloudChunk;

new_key_type! { pub struct InternalNodeId; }

#[derive(Clone, Copy, Default, Debug, Reflect, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[reflect(opaque)]
pub struct NodeId(InternalNodeId);

#[expect(
    unsafe_code,
    reason = "This implementation is safe because it only reuse [`slotmap::new_key_type`] generated impl."
)]
/// SAFETY: this implementation is safe because it only reuse [`slotmap::new_key_type`] generated
/// impl.
unsafe impl Key for NodeId {
    fn data(&self) -> KeyData {
        self.0.data()
    }
}

impl From<InternalNodeId> for NodeId {
    fn from(value: InternalNodeId) -> Self {
        NodeId(value)
    }
}

impl From<NodeId> for InternalNodeId {
    fn from(value: NodeId) -> Self {
        value.0
    }
}

impl From<KeyData> for NodeId {
    fn from(value: KeyData) -> Self {
        InternalNodeId::from(value).into()
    }
}

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

    pub const X_0_Y_0_Z_0: ChildIndex = ChildIndex(0b000);
    pub const X_0_Y_0_Z_1: ChildIndex = ChildIndex(0b001);
    pub const X_0_Y_1_Z_0: ChildIndex = ChildIndex(0b010);
    pub const X_0_Y_1_Z_1: ChildIndex = ChildIndex(0b011);
    pub const X_1_Y_0_Z_0: ChildIndex = ChildIndex(0b100);
    pub const X_1_Y_0_Z_1: ChildIndex = ChildIndex(0b101);
    pub const X_1_Y_1_Z_0: ChildIndex = ChildIndex(0b110);
    pub const X_1_Y_1_Z_1: ChildIndex = ChildIndex(0b111);
    pub const ROOT: ChildIndex = ChildIndex(0b1000);
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
        (0_u8..8).filter(move |&i| (self.bits() & (1 << i)) != 0)
    }
}

impl From<ChildIndex> for ChildrenMask {
    #[inline]
    fn from(value: ChildIndex) -> Self {
        ChildrenMask::from_bits_retain(1 << *value)
    }
}

#[derive(Clone, Debug)]
pub struct PointCloudNode {
    pub id: NodeId,
    /// The name of the node in the hierarchy with the following format:
    /// - "r" for the root node
    /// - "r" followed by the child index of the ancestors for a child node
    ///
    /// Examples: r, r0, r3, r4, r01, r07, r30, ...
    pub name: Arc<str>,
    pub point_count: usize,
    pub status: PointCloudNodeStatus,
    /// the child index of the current node
    pub child_index: ChildIndex,
    pub parent_id: Option<NodeId>,
    /// children indexed by their child index
    pub children: [NodeId; 8],
    /// the children mask stores, in binary form, the mask of existing children
    pub children_mask: ChildrenMask,
    pub aabb: Option<Aabb>,
    pub depth: u32,
    /// the custom data that the loader may reuse to load children
    pub data: NodeData,
    pub chunk: Option<Handle<PointCloudChunk>>,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum PointCloudNodeStatus {
    #[default]
    Proxy,
    Loading,
    Loaded,
}

#[derive(Clone, Debug, Deref)]
pub struct NodeData(pub Arc<dyn Any + Send + Sync + 'static>);

impl NodeData {
    pub fn new<T>(data: T) -> Self
    where
        T: Send + Sync + 'static,
    {
        Self(Arc::new(data))
    }
}

impl Default for NodeData {
    fn default() -> Self {
        Self(Arc::new(()))
    }
}
