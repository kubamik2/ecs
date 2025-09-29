use std::{alloc::Layout, marker::PhantomData, ptr::NonNull};
use crate::storage::{blob_vec::{BlobVec, Iter}, ptr::*, sparse_set::{SparseArray, SparseIndex}};

pub struct BlobSparseSet {
    sparse_array: SparseArray,
    dense: BlobVec,
    mapping: Vec<usize>,
}

#[allow(unused)]
impl BlobSparseSet {
    pub const fn new<T>() -> Self {
        Self {
            sparse_array: SparseArray::new(),
            dense: BlobVec::new::<T>(),
            mapping: vec![],
        }
    }

    #[inline]
    pub fn iter<T>(&self) -> Iter<'_, T> {
        self.dense.iter()
    }

    #[inline]
    pub fn contains(&self, id: usize) -> bool {
        let sparse_index = self.sparse_array.get(id);
        sparse_index.is_some()
    }

    /// # Safety
    /// Type T must be the same as the one used to create the BlobSparseSet
    #[inline]
    pub unsafe fn get<T>(&self, id: usize) -> Option<&T> {
        assert!(Layout::new::<T>() == self.dense.item_layout());
        let sparse_index = self.sparse_array.get(id);
        let index = sparse_index.get()?;
        Some(unsafe { self.dense.index(index).cast_ref::<T>() })
    }

    /// # Safety
    /// Type T must be the same as the one used to create the BlobSparseSet
    #[inline]
    pub unsafe fn get_mut<T>(&mut self, id: usize) -> Option<&mut T> {
        assert!(Layout::new::<T>() == self.dense.item_layout());
        let sparse_index = self.sparse_array.get(id);
        let index = sparse_index.get()?;
        Some(unsafe { self.dense.index_mut(index).cast_mut::<T>() })
    }

    #[inline]
    pub fn ptr(&self, id: usize) -> Ptr<'_> {
        let sparse_index = self.sparse_array.get(id);
        let index = sparse_index.0;
        self.dense.index(index)
    }

    #[inline]
    pub fn ptr_mut(&mut self, id: usize) -> PtrMut<'_> {
        let sparse_index = self.sparse_array.get(id);
        let index = sparse_index.0;
        self.dense.index_mut(index)
    }

    #[inline]
    pub fn get_ptr<'a>(&self, id: usize) -> Option<Ptr<'a>> {
        let sparse_index = self.sparse_array.get(id);
        let index = sparse_index.get()?;
        Some(self.dense.index(index))
    }

    #[inline]
    pub fn get_mut_ptr<'a>(&mut self, id: usize) -> Option<PtrMut<'a>> {
        let sparse_index = self.sparse_array.get(id);
        let index = sparse_index.get()?;
        Some(self.dense.index_mut(index))
    }

    /// # Safety
    /// Type T must be the same as the one used to create the BlobSparseSet
    #[inline]
    pub unsafe fn insert<T>(&mut self, id: usize, mut value: T) -> Option<T> {
        assert!(Layout::new::<T>() == self.dense.item_layout());
        let ptr = NonNull::from(&mut value).cast::<u8>();
        let (_, was_present) = unsafe { self.insert_ptr(id, ptr) };
        match was_present {
            true => Some(value),
            false => {
                std::mem::forget(value);
                None
            }
        }
    }

    /// # Safety
    /// ptr must point to a value of type used to create the BlobSparseSet
    /// Value must be properly aligned and valid for reads
    #[inline]
    unsafe fn insert_ptr(&mut self, id: usize, ptr: NonNull<u8>) -> (usize, bool) {
        let sparse_index = self.sparse_array.get(id); //        ^ was component present
        if let Some(index) = sparse_index.get() {
            let dst = self.dense.index_mut(index).as_ptr();
            let src = ptr.as_ptr();
            unsafe { std::ptr::swap_nonoverlapping(dst, src, self.dense.item_layout().size()) };
            (index, true)
        } else {
            let index = self.dense.len();
            self.sparse_array.set(id, SparseIndex::new(index));
            unsafe { self.dense.push(ptr) };
            self.mapping.push(id);
            (index, false)
        }
    }

    #[inline]
    fn insert_with_index<T>(&mut self, id: usize, mut value: T) -> usize {
        assert!(Layout::new::<T>() == self.dense.item_layout());
        let ptr = NonNull::from(&mut value).cast::<u8>();
        let index = unsafe { self.insert_ptr(id, ptr) }.0;
        std::mem::forget(value);
        index
    }

    #[inline]
    pub fn remove(&mut self, id: usize) {
        let sparse_index = self.sparse_array.get(id);
        let Some(index) = sparse_index.get() else { return; };

        let dense_len = self.dense.len();
        let back = self.mapping[dense_len-1];

        self.dense.swap(index, dense_len-1);
        self.mapping.swap(index, dense_len-1);

        self.sparse_array.set(back, sparse_index);
        self.sparse_array.set(id, SparseIndex::NONE);

        self.mapping.pop();
        self.dense.pop()
    }

    /// # Safety
    /// Type T must be the same as the one used to crete the BlobSparseSet
    #[inline]
    pub unsafe fn remove_as<T>(&mut self, id: usize) -> Option<T> {
        let sparse_index = self.sparse_array.get(id);
        let index = sparse_index.get()?;

        let dense_len = self.dense.len();
        let back = self.mapping[dense_len-1];

        self.dense.swap(index, dense_len-1);
        self.mapping.swap(index, dense_len-1);

        self.sparse_array.set(back, sparse_index);
        self.sparse_array.set(id, SparseIndex::NONE);

        self.mapping.pop();
        unsafe { self.dense.pop_as() }
    }

    fn remove_by_index(&mut self, id: usize, index: usize) {
        let dense_len = self.dense.len();
        let back = self.mapping[dense_len-1];

        self.dense.swap(index, dense_len-1);
        self.mapping.swap(index, dense_len-1);

        self.sparse_array.set(back, SparseIndex(index));
        self.sparse_array.set(id, SparseIndex::NONE);

        self.mapping.pop();
        self.dense.pop()
    }

    /// # Safety
    /// Type T must be the same as the one used to create the BlobSparseSet
    #[inline]
    pub unsafe fn entry<T>(&mut self, id: usize) -> Entry<'_, T> {
        assert!(Layout::new::<T>() == self.dense.item_layout());
        match self.sparse_array.get(id) {
            SparseIndex::NONE => Entry::Vacant(VacantEntry {
                sparse_set: self,
                id,
                _m: PhantomData,
            }),
            index => Entry::Occupied(OccupiedEntry {
                sparse_set: self,
                id,
                index: unsafe { index.get_unsafe() }, // previous match check ensures that this is safe
                _m: PhantomData,
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
    pub const fn len(&self) -> usize {
        self.dense.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.dense.len() == 0
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
    sparse_set: &'a mut BlobSparseSet,
    index: usize,
    id: usize,
    _m: PhantomData<T>,
}

impl<'a, V> OccupiedEntry<'a, V> {
    pub fn get(&self) -> &V {
        unsafe { self.sparse_set.dense.index(self.index).cast_ref::<V>() }
    }

    pub fn get_mut(&mut self) -> &mut V {
        unsafe { self.sparse_set.dense.index_mut(self.index).cast_mut::<V>() }
    }

    pub fn insert(&mut self, value: V) {
        unsafe { *self.sparse_set.dense.index_mut(self.index).cast_mut::<V>() = value };
    }

    pub fn into_mut(self) -> &'a mut V {
        unsafe { self.sparse_set.dense.index_mut(self.index).cast_mut::<V>() }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn remove(self) {
        self.sparse_set.remove_by_index(self.id, self.index);
    }
}

pub struct VacantEntry<'a, V> {
    sparse_set: &'a mut BlobSparseSet,
    id: usize,
    _m: PhantomData<V>,
}

impl<'a, V> VacantEntry<'a, V> {
    pub fn insert(self, value: V) -> &'a mut V {
        let index = self.sparse_set.insert_with_index(self.id, value);
        unsafe { self.sparse_set.dense.index_mut(index).cast_mut::<V>() }
    }

    pub fn insert_entry(self, value: V) -> OccupiedEntry<'a, V> {
        let index = self.sparse_set.insert_with_index(self.id, value);
        OccupiedEntry {
            sparse_set: self.sparse_set,
            id: self.id,
            index,
            _m: PhantomData,
        }
    }

    pub fn id(&self) -> usize {
        self.id
    }
}
