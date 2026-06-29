use std::{
    collections::BinaryHeap,
    ops::{Deref, DerefMut},
};

use super::StackedPointCloudNodeEntity;

/// A RAII guard to safely use a 'static [`BinaryHeap`] with short-lived references.
pub struct HeapGuard<'a, 'b> {
    // We hold a mutable reference to the heap, casted to the shorter lifetime.
    inner: &'b mut BinaryHeap<StackedPointCloudNodeEntity<'a>>,
}

#[expect(
    unsafe_code,
    reason = "This implementation fake the lifetime, but always clear the data on drop, to preserve allocations."
)]
impl<'a, 'b> HeapGuard<'a, 'b> {
    /// Creates the guard and transmutates the heap lifetime.
    ///
    /// # Safety
    /// This is safe because the Drop implementation ensures the heap is cleared
    /// before the references inside it (lifetime 'a) become invalid.
    pub fn new(storage: &'b mut BinaryHeap<StackedPointCloudNodeEntity<'static>>) -> Self {
        // SAFETY: [`inner`] is clear on drop (see [`<HeapGuard as Drop>::drop`])
        unsafe {
            // Memory layout of Node<'static> and Node<'a> is identical (pointer erasure).
            let transmuted = std::mem::transmute::<
                &mut BinaryHeap<StackedPointCloudNodeEntity<'static>>,
                &mut BinaryHeap<StackedPointCloudNodeEntity<'a>>,
            >(storage);

            Self { inner: transmuted }
        }
    }
}

// Automatically clears the heap when the guard goes out of scope.
impl<'a, 'b> Drop for HeapGuard<'a, 'b> {
    fn drop(&mut self) {
        self.inner.clear();
    }
}

// Deref allows you to use the guard exactly like a BinaryHeap.
impl<'a, 'b> Deref for HeapGuard<'a, 'b> {
    type Target = BinaryHeap<StackedPointCloudNodeEntity<'a>>;
    fn deref(&self) -> &Self::Target {
        self.inner
    }
}

impl<'a, 'b> DerefMut for HeapGuard<'a, 'b> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner
    }
}
