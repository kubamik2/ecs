use std::{any::Any, cell::SyncUnsafeCell, slice::Iter};

use super::Entity;

pub struct TypelessSparseSet {
    sparse_set: Box<dyn Any>,
    remove_ptr: fn(&mut Box<dyn Any>, Entity),
}

impl TypelessSparseSet {
    pub fn new<T: 'static>(sparse_set: SparseSet<T>) -> Self {
        Self {
            sparse_set: Box::new(SyncUnsafeCell::new(sparse_set)),
            remove_ptr: |sparse_set, entity| unsafe { sparse_set.downcast_mut::<SparseSet<T>>().unwrap_unchecked() }.remove(entity),
        }
    }

    #[inline]
    pub unsafe fn downcast_unchecked<T: 'static>(&self) -> &SyncUnsafeCell<SparseSet<T>> {
        unsafe { self.sparse_set.downcast_ref_unchecked::<SyncUnsafeCell<SparseSet<T>>>() }
    }

    #[inline]
    pub fn remove(&mut self, entity: Entity) {
        (self.remove_ptr)(&mut self.sparse_set, entity);
    }
}

pub struct SparseSet<T> {
    sparse_array: SparseArray,
    dense: Vec<T>,
    mapping: Vec<u16>,
}

impl<T> Default for SparseSet<T> {
    fn default() -> Self {
        Self {
            sparse_array: Default::default(),
            dense: vec![],
            mapping: vec![],
        }
    }
}

#[allow(unused)]
impl<T> SparseSet<T> {
    pub fn new() -> Self {
        Self {
            sparse_array: SparseArray::default(),
            dense: vec![],
            mapping: vec![],
        }
    }

    #[inline]
    pub fn iter(&self) -> Iter<T> {
        self.dense.iter()
    }

    #[inline]
    pub fn contains(&self, entity: Entity) -> bool {
        let sparse_index = self.sparse_array.get(entity.id());
        sparse_index.is_some()
    }

    #[inline]
    pub fn get(&self, entity: Entity) -> Option<&T> {
        let sparse_index = self.sparse_array.get(entity.id());
        let index = sparse_index.get()?;
        Some(&self.dense[index as usize])
    }

    #[inline]
    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
        let sparse_index = self.sparse_array.get(entity.id());
        let index = sparse_index.get()?;
        Some(&mut self.dense[index as usize])
    }

    #[inline]
    pub fn get_ptr_unchecked(&self, entity: Entity) -> *const T {
        let sparse_index = self.sparse_array.get(entity.id());
        let index = sparse_index.0 as usize;
        &self.dense[index]
    }

    #[inline]
    pub fn get_mut_ptr_unchecked(&mut self, entity: Entity) -> *mut T {
        let sparse_index = self.sparse_array.get(entity.id());
        let index = sparse_index.0 as usize;
        &mut self.dense[index]
    }

    #[inline]
    pub fn get_ptr(&self, entity: Entity) -> Option<*const T> {
        let sparse_index = self.sparse_array.get(entity.id());
        let index = sparse_index.get()?;
        Some(&self.dense[index as usize])
    }

    #[inline]
    pub fn get_mut_ptr(&mut self, entity: Entity) -> Option<*mut T> {
        let sparse_index = self.sparse_array.get(entity.id());
        let index = sparse_index.get()?;
        Some(&mut self.dense[index as usize])
    }

    #[inline]
    pub fn insert(&mut self, entity: Entity, value: T) {
        let sparse_index = self.sparse_array.get(entity.id());
        if let Some(index) = sparse_index.get() {
            self.dense[index as usize] = value;
        } else {
            self.sparse_array.set(entity.id(), SparseIndex::new(self.dense.len() as u16));
            self.dense.push(value);
            self.mapping.push(entity.id());
        }
    }

    #[inline]
    pub fn remove(&mut self, entity: Entity) {
        let sparse_index = self.sparse_array.get(entity.id());
        let Some(index) = sparse_index.get() else { return; };

        let dense_len = self.dense.len();
        let back = self.mapping[dense_len-1];
        self.dense.swap(index as usize, dense_len-1);
        self.mapping.swap(index as usize, dense_len-1);

        self.sparse_array.set(back, sparse_index);
        self.sparse_array.set(entity.id(), SparseIndex::NONE);

        self.dense.pop();
        self.mapping.pop();
    }
}

#[derive(Default)]
pub struct SparseArray(Vec<SparseIndex>);

impl SparseArray {
    #[inline]
    pub fn get(&self, entity_id: u16) -> SparseIndex {
        let id = entity_id as usize;
        self.0.get(id).copied().unwrap_or(SparseIndex::NONE)
    }

    #[inline]
    pub fn set(&mut self, entity_id: u16, index: SparseIndex) {
        let id = entity_id as usize;
        if id >= self.0.len() {
            self.0.resize(id+1, SparseIndex::NONE);
        }
        self.0[id] = index;
    }
}

/// # Safety
///
/// It's almost impossible for an index to hit u32::MAX
/// so it should be safe
#[derive(Clone, Copy)]
pub struct SparseIndex(u16);

impl std::fmt::Debug for SparseIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_some() {
            f.write_fmt(format_args!("{}", self.0))
        } else {
            f.write_str("None")
        }
    }
}

#[allow(unused)]
impl SparseIndex {
    pub const NONE: Self = Self(u16::MAX);

    #[inline]
    pub const fn new(index: u16) -> Self {
        Self(index)
    }

    #[inline]
    pub const fn is_some(&self) -> bool {
        self.0 != u16::MAX
    }
    
    #[inline]
    pub const fn is_none(&self) -> bool {
        self.0 == u16::MAX
    }

    #[inline]
    pub const fn get(&self) -> Option<u16> {
        if self.is_some() {
            Some(self.0)
        } else {
            None
        }
    }

    #[inline]
    pub const fn set(&mut self, index: u16) {
        self.0 = index;
    }
}
