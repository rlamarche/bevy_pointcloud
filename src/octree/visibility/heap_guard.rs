use std::collections::BinaryHeap;
use std::ops::{Deref, DerefMut};
use crate::octree::node::NodeData;
use crate::octree::visibility::stack::StackedOctreeNode;

/// A RAII guard to safely use a 'static BinaryHeap with short-lived references.
pub struct HeapGuard<'a, 'b, T: NodeData> {
    // We hold a mutable reference to the heap, casted to the shorter lifetime.
    inner: &'b mut BinaryHeap<StackedOctreeNode<'a, T>>,
}

impl<'a, 'b, T: NodeData> HeapGuard<'a, 'b, T> {
    /// Creates the guard and transmutates the heap lifetime.
    ///
    /// # Safety
    /// This is safe because the Drop implementation ensures the heap is cleared
    /// before the references inside it (lifetime 'a) become invalid.
    pub fn new(storage: &'b mut BinaryHeap<StackedOctreeNode<'static, T>>) -> Self {
        unsafe {
            // Memory layout of Node<'static> and Node<'a> is identical (pointer erasure).
            let transmuted = std::mem::transmute::<
                &mut BinaryHeap<StackedOctreeNode<'static, T>>,
                &mut BinaryHeap<StackedOctreeNode<'a, T>>
            >(storage);

            Self { inner: transmuted }
        }
    }
}

// Automatically clears the heap when the guard goes out of scope.
impl<'a, 'b, T: NodeData> Drop for HeapGuard<'a, 'b, T> {
    fn drop(&mut self) {
        self.inner.clear();
    }
}

// Deref allows you to use the guard exactly like a BinaryHeap.
impl<'a, 'b, T: NodeData> Deref for HeapGuard<'a, 'b, T> {
    type Target = BinaryHeap<StackedOctreeNode<'a, T>>;
    fn deref(&self) -> &Self::Target { self.inner }
}

impl<'a, 'b, T: NodeData> DerefMut for HeapGuard<'a, 'b, T> {
    fn deref_mut(&mut self) -> &mut Self::Target { self.inner }
}
