pub mod blob_sparse_set;
use std::{any::Any, cell::SyncUnsafeCell, slice::Iter};

pub struct TypelessSparseSet {
    sparse_set: Box<dyn Any>,
    remove_ptr: fn(&mut Box<dyn Any>, usize),
}

impl TypelessSparseSet {
    pub fn new<T: 'static>(sparse_set: SparseSet<T>) -> Self {
        Self {
            sparse_set: Box::new(SyncUnsafeCell::new(sparse_set)),
            remove_ptr: |sparse_set, id| {
                unsafe { sparse_set.downcast_mut_unchecked::<SyncUnsafeCell<SparseSet<T>>>().get_mut() }.remove(id);
            },
        }
    }

    #[inline]
    pub unsafe fn downcast_unchecked<T: 'static>(&self) -> &SyncUnsafeCell<SparseSet<T>> { unsafe { self.sparse_set.downcast_ref_unchecked::<SyncUnsafeCell<SparseSet<T>>>() }
    }

    #[inline]
    pub fn remove(&mut self, id: usize) {
        (self.remove_ptr)(&mut self.sparse_set, id);
    }
}

pub const SPARSE_SET_CAPACITY: usize = SparseIndex::MAX;

pub struct SparseSet<T> {
    sparse_array: SparseArray,
    dense: Vec<T>,
    mapping: Vec<usize>,
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
    pub fn contains(&self, id: usize) -> bool {
        let sparse_index = self.sparse_array.get(id);
        sparse_index.is_some()
    }

    #[inline]
    pub fn get(&self, id: usize) -> Option<&T> {
        let sparse_index = self.sparse_array.get(id);
        let index = sparse_index.get()?;
        Some(&self.dense[index])
    }

    #[inline]
    pub fn get_mut(&mut self, id: usize) -> Option<&mut T> {
        let sparse_index = self.sparse_array.get(id);
        let index = sparse_index.get()?;
        Some(&mut self.dense[index])
    }

    #[inline]
    pub fn ptr(&self, id: usize) -> *const T {
        let sparse_index = self.sparse_array.get(id);
        let index = sparse_index.0;
        &self.dense[index]
    }

    #[inline]
    pub fn ptr_mut(&mut self, id: usize) -> *mut T {
        let sparse_index = self.sparse_array.get(id);
        let index = sparse_index.0;
        &mut self.dense[index]
    }

    #[inline]
    pub fn get_ptr(&self, id: usize) -> Option<*const T> {
        let sparse_index = self.sparse_array.get(id);
        let index = sparse_index.get()?;
        Some(&self.dense[index])
    }

    #[inline]
    pub fn get_mut_ptr(&mut self, id: usize) -> Option<*mut T> {
        let sparse_index = self.sparse_array.get(id);
        let index = sparse_index.get()?;
        Some(&mut self.dense[index])
    }

    #[inline]
    pub fn insert(&mut self, id: usize, mut value: T) -> Option<T> {
        let sparse_index = self.sparse_array.get(id);
        if let Some(index) = sparse_index.get() {
            std::mem::swap(&mut value, &mut self.dense[index]);
            Some(value)
        } else {
            self.sparse_array.set(id, SparseIndex::new(self.dense.len()));
            self.dense.push(value);
            self.mapping.push(id);
            None
        }
    }

    #[inline]
    fn insert_with_index(&mut self, id: usize, value: T) -> usize {
        let sparse_index = self.sparse_array.get(id);
        if let Some(index) = sparse_index.get() {
            self.dense[index] = value;
            index
        } else {
            let index = self.dense.len();
            self.sparse_array.set(id, SparseIndex::new(index));
            self.dense.push(value);
            self.mapping.push(id);
            index
        }
    }

    #[inline]
    pub fn remove(&mut self, id: usize) -> Option<T> {
        let sparse_index = self.sparse_array.get(id);
        let index = sparse_index.get()?;

        let dense_len = self.dense.len();
        let back = self.mapping[dense_len-1];
        self.dense.swap(index, dense_len-1);
        self.mapping.swap(index, dense_len-1);

        self.sparse_array.set(back, sparse_index);
        self.sparse_array.set(id, SparseIndex::NONE);

        self.mapping.pop();
        self.dense.pop()
    }

    #[inline]
    unsafe fn remove_by_index_unchecked(&mut self, id: usize, index: usize) -> T {
        let dense_len = self.dense.len();
        let back = self.mapping[dense_len-1];
        self.dense.swap(index, dense_len-1);
        self.mapping.swap(index, dense_len-1);

        self.sparse_array.set(back, SparseIndex(index));
        self.sparse_array.set(id, SparseIndex::NONE);

        self.mapping.pop();
        unsafe { self.dense.pop().unwrap_unchecked() }
    }

    #[inline]
    pub fn entry(&mut self, id: usize) -> Entry<'_, T> {
        match self.sparse_array.get(id) {
            SparseIndex::NONE => Entry::Vacant(VacantEntry {
                sparse_set: self,
                id,
            }),
            index => Entry::Occupied(OccupiedEntry {
                sparse_set: self,
                id,
                index: unsafe { index.get_unsafe() }, // previous match check ensures that this is safe
            }),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.sparse_array.clear();
        self.mapping.clear();
        self.dense.clear();
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.dense.len()
    }
}

#[derive(Default)]
pub struct SparseArray(Vec<SparseIndex>);

impl SparseArray {
    #[inline]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    #[inline]
    pub fn get(&self, id: usize) -> SparseIndex {
        self.0.get(id).copied().unwrap_or(SparseIndex::NONE)
    }

    #[inline]
    pub fn set(&mut self, id: usize, index: SparseIndex) {
        if id >= self.0.len() {
            self.0.resize(id+1, SparseIndex::NONE);
        }
        self.0[id] = index;
    }

    #[inline]
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SparseIndex(usize);

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
    pub const NONE: Self = Self(usize::MAX);
    pub const MAX: usize = usize::MAX - 1;

    #[inline]
    const fn new(index: usize) -> Self {
        Self(index)
    }

    #[inline]
    const fn is_some(&self) -> bool {
        self.0 != usize::MAX
    }
    
    #[inline]
    const fn is_none(&self) -> bool {
        self.0 == usize::MAX
    }

    #[inline]
    const fn get(&self) -> Option<usize> {
        if self.is_some() {
            Some(self.0)
        } else {
            None
        }
    }

    #[inline]
    const fn set(&mut self, index: usize) {
        self.0 = index;
    }

    #[inline]
    const unsafe fn get_unsafe(&self) -> usize {
        self.0
    }
}

pub enum Entry<'a, V> {
    Occupied(OccupiedEntry<'a, V>),
    Vacant(VacantEntry<'a, V>),
}

impl<'a, V> Entry<'a, V> {
    #[inline]
    pub fn and_modify<F: FnOnce(&mut V)>(self, f: F) -> Self {
        match self {
            Entry::Occupied(mut entry) => {
                f(entry.get_mut());
                Entry::Occupied(entry)
            },
            Entry::Vacant(entry) => Entry::Vacant(entry),
        }
    }

    pub fn insert(self, value: V) -> OccupiedEntry<'a, V> {
        match self {
            Entry::Occupied(mut entry) => {
                entry.insert(value);
                entry
            },
            Entry::Vacant(entry) => entry.insert_entry(value),
        }
    }

    pub fn id(&self) -> usize {
        match self {
            Entry::Occupied(entry) => entry.id,
            Entry::Vacant(entry) => entry.id,
        }
    }

    pub fn or_insert(self, default: V) -> &'a mut V {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default),
        }
    }

    pub fn or_insert_with<F: FnOnce() -> V>(self, default: F) -> &'a mut V {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default()),
        }
    }

    pub fn or_insert_with_id<F: FnOnce(usize) -> V>(self, default: F) -> &'a mut V {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let value = default(entry.id());
                entry.insert(value)
            }
        }
    }
}

impl<'a, V: Default> Entry<'a, V> {
    pub fn or_default(self) -> &'a mut V {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(Default::default())
        }
    }
}

pub struct OccupiedEntry<'a, T> {
    sparse_set: &'a mut SparseSet<T>,
    index: usize,
    id: usize,
}

impl<'a, V> OccupiedEntry<'a, V> {
    pub fn get(&self) -> &V {
        &self.sparse_set.dense[self.index]
    }

    pub fn get_mut(&mut self) -> &mut V {
        &mut self.sparse_set.dense[self.index]
    }

    pub fn insert(&mut self, mut value: V) -> V {
        std::mem::swap(&mut value, &mut self.sparse_set.dense[self.index]);
        value
    }

    pub fn into_mut(self) -> &'a mut V {
        &mut self.sparse_set.dense[self.index]
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn remove(self) -> V {
        unsafe { self.sparse_set.remove_by_index_unchecked(self.id, self.index) }
    }
}

pub struct VacantEntry<'a, V> {
    sparse_set: &'a mut SparseSet<V>,
    id: usize,
}

impl<'a, V> VacantEntry<'a, V> {
    pub fn insert(self, value: V) -> &'a mut V {
        let index = self.sparse_set.insert_with_index(self.id, value);
        &mut self.sparse_set.dense[index]
    }

    pub fn insert_entry(self, value: V) -> OccupiedEntry<'a, V> {
        let index = self.sparse_set.insert_with_index(self.id, value);
        OccupiedEntry {
            sparse_set: self.sparse_set,
            id: self.id,
            index
        }
    }

    pub fn id(&self) -> usize {
        self.id
    }
}
