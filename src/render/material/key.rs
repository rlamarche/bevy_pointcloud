use std::{
    any::{Any, TypeId},
    hash::{BuildHasher, Hash, Hasher},
    sync::Arc,
};

use bevy::platform::hash::FixedHasher;

#[derive(Debug)]
pub struct ErasedViewSettingsKey {
    type_id: TypeId,
    hash: u64,
    value: Box<dyn Any + Send + Sync>,
    vtable: Arc<ErasedViewSettingsKeyVTable>,
}

#[derive(Debug)]
pub struct ErasedViewSettingsKeyVTable {
    clone_fn: fn(&dyn Any) -> Box<dyn Any + Send + Sync>,
    partial_eq_fn: fn(&dyn Any, &dyn Any) -> bool,
}

impl ErasedViewSettingsKey {
    pub fn new<T>(view_settings: T) -> Self
    where
        T: Clone + Hash + PartialEq + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        let hash = FixedHasher::hash_one(&FixedHasher, &view_settings);

        fn clone<T: Clone + Send + Sync + 'static>(any: &dyn Any) -> Box<dyn Any + Send + Sync> {
            Box::new(any.downcast_ref::<T>().unwrap().clone())
        }
        fn partial_eq<T: PartialEq + 'static>(a: &dyn Any, b: &dyn Any) -> bool {
            a.downcast_ref::<T>().unwrap() == b.downcast_ref::<T>().unwrap()
        }

        Self {
            type_id,
            hash,
            value: Box::new(view_settings),
            vtable: Arc::new(ErasedViewSettingsKeyVTable {
                clone_fn: clone::<T>,
                partial_eq_fn: partial_eq::<T>,
            }),
        }
    }

    pub fn to_key<T: Clone + 'static>(&self) -> T {
        debug_assert_eq!(self.type_id, TypeId::of::<T>());
        self.value.downcast_ref::<T>().unwrap().clone()
    }
}

impl PartialEq for ErasedViewSettingsKey {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id
            && (self.vtable.partial_eq_fn)(self.value.as_ref(), other.value.as_ref())
    }
}

impl Eq for ErasedViewSettingsKey {}

impl Clone for ErasedViewSettingsKey {
    fn clone(&self) -> Self {
        Self {
            type_id: self.type_id,
            hash: self.hash,
            value: (self.vtable.clone_fn)(self.value.as_ref()),
            vtable: self.vtable.clone(),
        }
    }
}

impl Hash for ErasedViewSettingsKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.type_id.hash(state);
        self.hash.hash(state);
    }
}

impl Default for ErasedViewSettingsKey {
    fn default() -> Self {
        Self::new(())
    }
}
