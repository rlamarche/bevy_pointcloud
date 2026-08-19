use std::{any::Any, sync::Arc};

use bevy::{
    asset::Handle,
    camera::primitives::Aabb,
    prelude::Deref,
    reflect::{std_traits::ReflectDefault, Reflect, ReflectDeserialize, ReflectSerialize},
};
use serde::{Deserialize, Serialize};

use crate::{impl_node_id_wrapper, PointCloudChunk};

pub mod wrapper {
    use slotmap::new_key_type;

    new_key_type! { pub struct InternalNodeId; }

    #[macro_export]
    macro_rules! impl_node_id_wrapper {
        ($name:ident) => {
            #[derive(Clone, Copy, Default, Debug, bevy::reflect::Reflect, PartialEq, Eq, PartialOrd, Ord, Hash)]
            #[reflect(opaque)]
            pub struct $name($crate::wrapper::InternalNodeId);

            impl $name {
                /// Returns a null node identifier.
                ///
                /// This is typically used as a sentinel value to represent the absence
                /// of a node, such as for uninitialized or non-existent child nodes.
                pub fn null() -> Self {
                    Self(slotmap::Key::null())
                }

                pub fn is_null(&self) -> bool {
                    slotmap::Key::is_null(&self.0)
                }
            }

            #[expect(
                unsafe_code,
                reason = "This implementation is safe because it only reuse [`slotmap::new_key_type`] generated impl."
            )]
            /// SAFETY: this implementation is safe because it only reuse [`slotmap::new_key_type`] generated
            /// impl.
            unsafe impl slotmap::Key for $name {
                fn data(&self) -> slotmap::KeyData {
                    self.0.data()
                }
            }

            impl From<$crate::wrapper::InternalNodeId> for $name {
                fn from(value: $crate::wrapper::InternalNodeId) -> Self {
                    $name(value)
                }
            }

            impl From<$name> for $crate::wrapper::InternalNodeId {
                fn from(value: $name) -> Self {
                    value.0
                }
            }

            impl From<slotmap::KeyData> for $name {
                fn from(value: slotmap::KeyData) -> Self {
                    $crate::wrapper::InternalNodeId::from(value).into()
                }
            }
        };
    }
}

impl_node_id_wrapper!(NodeId);

#[derive(Copy, Clone, Debug, Default, Deref, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

    /// Returns child index if valid
    #[inline]
    pub fn index(&self) -> Option<u8> {
        if self.0 > 0b111 {
            return None;
        }
        Some(self.0)
    }

    /// Returns unchecked index
    #[inline]
    pub fn raw_index(&self) -> u8 {
        self.0
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
    pub const NONE: ChildIndex = ChildIndex(u8::MAX);
}

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Default, Hash, Clone, Copy, PartialEq, Eq, Debug, Reflect, Serialize, Deserialize)]
    #[reflect(opaque, Default, Hash, Clone, PartialEq, Debug, Serialize, Deserialize)]
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
    pub fn has_child(&self, child_index: ChildIndex) -> bool {
        (self.bits()
            & (1_u8
                << child_index
                    .index()
                    .expect("Trying to convert child index which isn't a valid index.")))
            > 0
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
    pub status: PointCloudNodeStatus,
    pub point_count: usize,
    /// the child index of the current node
    pub child_index: ChildIndex,
    pub parent_id: Option<NodeId>,
    /// children indexed by their child index
    pub children: [NodeId; 8],
    /// the children mask stores, in binary form, the mask of existing children
    pub children_mask: ChildrenMask,
    pub aabb: Option<Aabb>,
    pub depth: u32,
    pub offset: Option<f32>,
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
