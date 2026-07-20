use std::{
    any::TypeId,
    hash::{Hash, Hasher},
};

/// A type-erased mesh pipeline key, which stores the bits of the key as a `u64`.
#[derive(Clone, Copy)]
pub struct ErasedSplatPipelineKey {
    bits: u64,
    type_id: TypeId,
}

impl ErasedSplatPipelineKey {
    #[inline]
    pub fn new<T: 'static>(key: T) -> Self
    where
        u64: From<T>,
    {
        Self {
            bits: key.into(),
            type_id: TypeId::of::<T>(),
        }
    }

    #[inline]
    pub fn downcast<T: 'static + From<u64>>(&self) -> T {
        assert_eq!(
            self.type_id,
            TypeId::of::<T>(),
            "ErasedSplatPipelineKey::downcast called with wrong type"
        );
        self.bits.into()
    }
}

impl PartialEq for ErasedSplatPipelineKey {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id && self.bits == other.bits
    }
}

impl Eq for ErasedSplatPipelineKey {}

impl Hash for ErasedSplatPipelineKey {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.type_id.hash(state);
        self.bits.hash(state);
    }
}

impl Default for ErasedSplatPipelineKey {
    fn default() -> Self {
        Self {
            bits: 0,
            type_id: TypeId::of::<()>(),
        }
    }
}

impl core::fmt::Debug for ErasedSplatPipelineKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ErasedSplatPipelineKey")
            .field("type_id", &self.type_id)
            .field("bits", &format_args!("{:#018x}", self.bits))
            .finish()
    }
}
