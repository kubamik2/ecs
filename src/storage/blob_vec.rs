use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::{alloc::Layout, ptr::NonNull};
use std::alloc::{handle_alloc_error, Allocator, Global};

use super::ptr::{Ptr, PtrMut};

pub struct BlobVec<A: Allocator = Global> {
    raw: RawBlobVec<A>,
    len: usize,
    item_layout: Layout,
    drop: unsafe fn(NonNull<u8>),
}

impl BlobVec<Global> {
    #[inline]
    pub const fn new<T>() -> BlobVec<Global> {
        BlobVec {
            raw: RawBlobVec::new(Global),
            len: 0,
            item_layout: Layout::new::<T>(),
            drop: |ptr| {
                let ptr = ptr.cast::<T>();
                unsafe { ptr.drop_in_place() };
            },
        }
    }
}

impl<A: Allocator> BlobVec<A> {
    #[inline]
    pub const fn new_in<T>(alloc: A) -> Self {
        Self {
            raw: RawBlobVec::new(alloc),
            len: 0,
            item_layout: Layout::new::<T>(),
            drop: |ptr| {
                let ptr = ptr.cast::<T>();
                unsafe { ptr.drop_in_place() };
            },
        }
    }

    /// Pushes the copied pointed value into the BlobVec
    /// # Safety
    /// Value pointed by ptr must be of the same type that was used to create the BlobVec.
    /// ptr must contain valid data
    #[inline]
    pub unsafe fn push(&mut self, ptr: NonNull<u8>) {
        let len = self.len;

        if len == self.raw.capacity {
            self.raw.grow_one(self.item_layout);
        }

        let size = self.item_layout.size();
        unsafe {
            self.raw.ptr.add(self.len * size).copy_from_nonoverlapping(ptr, self.item_layout.size());
            self.len += 1;
        }
    }

    #[inline]
    pub fn get<'a>(&mut self, index: usize) -> Option<Ptr<'a>> {
        let len = self.len;
        if index >= len {
            return None;
        }

        let size = self.item_layout.size();
        let ptr = unsafe { self.raw.ptr.add(index * size) };
        Some(Ptr::new(ptr))
    }


    #[inline]
    pub fn get_mut<'a>(&mut self, index: usize) -> Option<PtrMut<'a>> {
        let len = self.len;
        if index >= len {
            return None;
        }

        let size = self.item_layout.size();
        let ptr = unsafe { self.raw.ptr.add(index * size) };
        Some(PtrMut::new(ptr))
    }

    #[inline]
    pub fn iter<'a, T>(&self) -> Iter<'a, T> {
        Iter {
            ptr: self.raw.ptr.cast::<T>(),
            len: self.len,
            _m: PhantomData,
        }
    }

    #[inline]
    pub fn index<'a>(&self, index: usize) -> Ptr<'a> {
        assert!(index < self.len);
        let size = self.item_layout.size();
        let ptr = unsafe { self.raw.ptr.add(index * size) };
        Ptr::new(ptr)
    }

    #[inline]
    pub fn index_mut<'a>(&mut self, index: usize) -> PtrMut<'a> {
        assert!(index < self.len);
        let size = self.item_layout.size();
        let ptr = unsafe { self.raw.ptr.add(index * size) };
        PtrMut::new(ptr)
    }

    #[inline]
    pub const fn item_layout(&self) -> Layout {
        self.item_layout
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn pop(&mut self) {
        if self.len == 0 { return; }
        self.len -= 1;
        let size = self.item_layout.size();
        unsafe {
            let ptr = self.raw.ptr.add(self.len * size);
            (self.drop)(ptr)
        }
    }

    /// # Safety
    /// Type T must be the same as the one used to create the BlobVec
    #[inline]
    pub unsafe fn pop_as<T>(&mut self) -> Option<T> {
        if self.len == 0 { return None; }
        self.len -= 1;
        unsafe {
            let mut value = MaybeUninit::<T>::uninit();
            let ptr = self.raw.ptr.cast::<T>().add(self.len);
            value.write(ptr.read());
            Some(value.assume_init())
        }
    }

    #[inline]
    pub fn swap(&mut self, a: usize, b: usize) {
        assert!(a < self.len && b < self.len);
        let size = self.item_layout.size();
        let mut ptr_a = unsafe { self.raw.ptr.add(a * size) };
        let mut ptr_b = unsafe { self.raw.ptr.add(b * size) };
        for _ in 0..self.item_layout.size() {
            unsafe {
                ptr_a.swap(ptr_b); 
                ptr_a = ptr_a.add(1);
                ptr_b = ptr_b.add(1);
            }
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        let mut ptr = self.raw.ptr;
        let size = self.item_layout.size();
        for _ in 0..self.len {
            unsafe {
                (self.drop)(ptr);
                ptr = ptr.add(size);
            }
        }
        self.len = 0;
    }
}

pub struct Iter<'a, T> {
    ptr: NonNull<T>,
    len: usize,
    _m: PhantomData<&'a T>
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 { return None; }
        self.len -= 1;
        unsafe {
            let val = self.ptr.as_ref();
            self.ptr = self.ptr.add(1);
            Some(val)
        }
    }
}

struct RawBlobVec<A: Allocator = Global> {
    ptr: NonNull<u8>,
    capacity: usize,
    alloc: A,
}

impl<A: Allocator> RawBlobVec<A> {
    pub const fn new(alloc: A) -> Self {
        Self {
            ptr: NonNull::dangling(),
            capacity: 0,
            alloc
        }
    }

    fn grow_amortized(&mut self, len: usize, additional: usize, item_layout: Layout) {
        if item_layout.size() == 0 {
            return;
        }
        assert!(additional > 0);

        let required_cap = len + additional;

        let cap = std::cmp::max(self.capacity * 2, required_cap);
        let cap = std::cmp::max(cap, 1);

        let new_layout = item_layout.repeat(cap).expect("BlobVec capacity overflow").0;

        match self.capacity {
            0 => {
                self.ptr = self.alloc.allocate(new_layout).unwrap_or_else(|_| handle_alloc_error(new_layout)).cast();
            },
            _ => {
                let old_layout = item_layout.repeat(self.capacity).expect("BlobVec capacity overflow").0;
                self.ptr = unsafe { self.alloc.grow(self.ptr, old_layout, new_layout) }.unwrap_or_else(|_| handle_alloc_error(new_layout)).cast::<u8>();
            }
        }
        self.capacity = cap;
    }

    fn grow_one(&mut self, item_layout: Layout) {
        self.grow_amortized(self.capacity, 1, item_layout);
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use std::marker::PhantomData;
    use std::mem::MaybeUninit;
    use std::ptr::NonNull;
    use std::str::FromStr;

    use super::BlobVec;

    fn verify_state<T: PartialEq>(state: &BlobVec, expected: &[T]) {
        for i in 0..state.len() {
            assert!(unsafe { state.index(i).cast_ref::<T>() } == &expected[i], "state invalid at {}", i);
        }

        for (i, val) in state.iter::<T>().enumerate() {
            assert!(val == &expected[i], "state iter invalid at {}", i);
        }
    }

    #[test]
    fn blob_vec_simple() {unsafe {
        let mut state = BlobVec::new::<i32>();
        state.push(NonNull::from(&-1).cast());
        state.push(NonNull::from(&-2).cast());
        state.push(NonNull::from(&-3).cast());
        verify_state::<i32>(&state, &[-1, -2, -3]);
        state.pop();
        verify_state::<i32>(&state, &[-1, -2]);
        state.clear();
        verify_state::<i32>(&state, &[]);
        for i in 0..10 {
            state.push(NonNull::from(&-i).cast());
        }
        verify_state::<i32>(&state, &[0,-1,-2,-3,-4,-5,-6,-7,-8,-9]);
        for i in (1..=10).rev() {
            verify_state::<i32>(&state, &(0..i).map(|i| -i).collect::<Vec<i32>>());
            state.pop();
        }
        state.pop();
        verify_state::<i32>(&state, &[]);
        state.push(NonNull::from(&123).cast());
        let val = state.pop_as::<i32>();
        assert!(val == Some(123));
    }}

    #[test]
    fn blob_vec_swap_simple() {unsafe {
        let mut state = BlobVec::new::<i32>();
        state.push(NonNull::from(&1).cast());
        state.push(NonNull::from(&-2).cast());
        state.push(NonNull::from(&3).cast());
        state.push(NonNull::from(&-4).cast());
        state.push(NonNull::from(&5).cast());
        state.swap(0, 0);
        state.swap(0, 3);
        state.swap(1, 2);
        state.swap(0, 4);
        verify_state::<i32>(&state, &[5, 3, -2, 1, -4]);
        state.clear();
        assert!(state.is_empty());
    }}

    #[derive(Debug, Clone)]
    struct Complex {
        message: String,
        padding: MaybeUninit<u8>,
        _m: PhantomData<()>,
        bytes: Box<[u8; 128]>,
        padding2: MaybeUninit<u128>,
        cstr: CString,
    }

    impl PartialEq for Complex {
        fn eq(&self, other: &Self) -> bool {
            self.message == other.message &&
            self.bytes == other.bytes &&
            self.cstr == other.cstr
        }
    }

    fn push_complex(state: &mut BlobVec, message: &str, bytes: Box<[u8; 128]>, cstr: &str) {
        let val = complex(message, bytes, cstr);
        unsafe { state.push(NonNull::from(&val).cast()) };
        std::mem::forget(val);
    }

    fn complex(message: &str, bytes: Box<[u8; 128]>, cstr: &str) -> Complex {
        Complex {
            message: message.into(),
            padding: MaybeUninit::uninit(),
            _m: PhantomData,
            bytes,
            padding2: MaybeUninit::uninit(),
            cstr: CString::from_str(cstr).unwrap(),
        }
    }
    
    #[test]
    fn blob_vec_complex() {
        let mut state = BlobVec::new::<Complex>();
        push_complex(&mut state, "Complex A", Box::new(std::array::from_fn(|i| i as u8)), "Complex A");
        push_complex(&mut state, "Complex B", Box::new(std::array::from_fn(|i| (i+1) as u8)), "Complex B");
        push_complex(&mut state, "Complex C", Box::new(std::array::from_fn(|i| (i+2) as u8)), "Complex C");
        let a = complex("Complex A", Box::new(std::array::from_fn(|i| i as u8)), "Complex A");
        let b = complex("Complex B", Box::new(std::array::from_fn(|i| (i+1) as u8)), "Complex B");
        let c = complex("Complex C", Box::new(std::array::from_fn(|i| (i+2) as u8)), "Complex C");

        verify_state(&state, &[a.clone(), b.clone(), c.clone()]);
        assert!(unsafe { state.index(1).cast_ref::<Complex>() } == &b);

        state.pop();
        verify_state(&state, &[a.clone(), b.clone()]);
        state.clear();

        push_complex(&mut state, "Complex A", Box::new(std::array::from_fn(|i| i as u8)), "Complex A");
        push_complex(&mut state, "Complex B", Box::new(std::array::from_fn(|i| (i+1) as u8)), "Complex B");
        let b_popped = unsafe { state.pop_as::<Complex>() };
        let a_popped = unsafe { state.pop_as::<Complex>() };
        assert!(a_popped == Some(a));
        assert!(b_popped == Some(b));
        assert!(unsafe { state.pop_as::<Complex>() }.is_none());
    }

    #[test]
    fn blob_vec_swap_complex() {
        let a = complex("Complex A", Box::new(std::array::from_fn(|i| i as u8)), "Complex A");
        let b = complex("Complex B", Box::new(std::array::from_fn(|i| (i+1) as u8)), "Complex B");
        let c = complex("Complex C", Box::new(std::array::from_fn(|i| (i+2) as u8)), "Complex C");
        let d = complex("Complex D", Box::new(std::array::from_fn(|i| (i+3) as u8)), "Complex D");
        let e = complex("Complex E", Box::new(std::array::from_fn(|i| (i+4) as u8)), "Complex E");
        let mut state = BlobVec::new::<Complex>();
        push_complex(&mut state, "Complex A", Box::new(std::array::from_fn(|i| i as u8)), "Complex A");
        push_complex(&mut state, "Complex B", Box::new(std::array::from_fn(|i| (i+1) as u8)), "Complex B");
        push_complex(&mut state, "Complex C", Box::new(std::array::from_fn(|i| (i+2) as u8)), "Complex C");
        push_complex(&mut state, "Complex D", Box::new(std::array::from_fn(|i| (i+3) as u8)), "Complex D");
        push_complex(&mut state, "Complex E", Box::new(std::array::from_fn(|i| (i+4) as u8)), "Complex E");
        state.swap(0, 0);
        state.swap(0, 3);
        state.swap(1, 2);
        state.swap(0, 4);
        verify_state::<Complex>(&state, &[e, c, b, a, d]);
        state.clear();
        assert!(state.is_empty());
    }

    #[test]
    fn blob_vec_zero_sized() {
        let mut state = BlobVec::new::<()>();
        unsafe { state.push(NonNull::from(&()).cast()) };
        unsafe { state.push(NonNull::from(&()).cast()) };
        unsafe { state.push(NonNull::from(&()).cast()) };
        assert!(state.get(0).is_some());
        assert!(state.get(1).is_some());
        assert!(state.get(2).is_some());
    }
}
