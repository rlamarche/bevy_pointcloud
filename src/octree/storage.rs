/// Generated using claude.ai

use slab::Slab;

/// A stable handle used to reference an entry inside a `GenerationalSlab`.
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct NodeId {
    pub index: usize,
    pub generation: usize,
}

/// A wrapper around `slab::Slab` that adds generation tracking.
/// This ensures that old `NodeId`s become invalid once a slot is reused.
#[derive(Debug, Clone)]
pub struct GenerationalSlab<T> {
    slab: Slab<(usize, T)>,     // (generation, value)
    generations: Vec<usize>,    // current generation per index
}

impl<T> GenerationalSlab<T> {
    /// Creates an empty `GenerationalSlab`.
    pub fn new() -> Self {
        Self {
            slab: Slab::new(),
            generations: Vec::new(),
        }
    }

    /// Inserts a new value into the slab.
    /// Returns a `NodeId` with the current generation for that slot.
    pub fn insert(&mut self, value: T) -> NodeId {
        // Insert a placeholder; we'll update its generation afterward.
        let index = self.slab.insert((0, value));

        // Make sure `generations` is large enough.
        if self.generations.len() <= index {
            self.generations.resize(index + 1, 0);
        }

        // Get the current generation for this index.
        let generation_value = self.generations[index];

        // Update the generation stored in the slab entry.
        if let Some((stored_generation, _)) = self.slab.get_mut(index) {
            *stored_generation = generation_value;
        }

        NodeId {
            index,
            generation: generation_value,
        }
    }

    /// Returns a reference to the value if the generation matches.
    pub fn get(&self, id: NodeId) -> Option<&T> {
        self.slab
            .get(id.index)
            .and_then(|(stored_generation, value)| {
                if *stored_generation == id.generation {
                    Some(value)
                } else {
                    None
                }
            })
    }

    /// Returns a mutable reference to the value if the generation matches.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut T> {
        self.slab
            .get_mut(id.index)
            .and_then(|(stored_generation, value)| {
                if *stored_generation == id.generation {
                    Some(value)
                } else {
                    None
                }
            })
    }

    /// Removes the value associated with the given `NodeId`,
    /// only if its generation matches.
    ///
    /// After removal, the generation for that index is incremented
    /// to prevent stale handles from remaining valid.
    pub fn remove(&mut self, id: NodeId) -> Option<T> {
        if let Some((stored_generation, _)) = self.slab.get(id.index) {
            if *stored_generation == id.generation {
                // Increment generation before reusing the slot later.
                self.generations[id.index] =
                    self.generations[id.index].wrapping_add(1);

                // Remove and return the stored value.
                let (_old_generation, value) = self.slab.remove(id.index);
                return Some(value);
            }
        }
        None
    }

    /// Returns an iterator over all valid entries.
    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &T)> {
        self.slab.iter().map(move |(index, (generation_value, value))| {
            (
                NodeId {
                    index,
                    generation: *generation_value,
                },
                value,
            )
        })
    }

    /// Returns the number of active entries.
    pub fn len(&self) -> usize {
        self.slab.len()
    }

    /// Returns `true` if there are no active entries.
    pub fn is_empty(&self) -> bool {
        self.slab.is_empty()
    }
}
