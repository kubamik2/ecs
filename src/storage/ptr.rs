use std::{marker::PhantomData, ptr::NonNull};

#[derive(Clone, Copy)]
pub struct Ptr<'a> {
    ptr: NonNull<u8>,
    _m: PhantomData<(&'a mut u8, u8)>,
}

impl<'a> Ptr<'a> {
    #[inline]
    pub const fn new(ptr: NonNull<u8>) -> Self {
        Self {
            ptr,
            _m: PhantomData,
        }
    }

    /// # Safety
    /// must point to type T
    #[inline]
    pub const unsafe fn cast_ref<T>(&self) -> &'a T {
        unsafe { self.ptr.cast::<T>().as_ref() }
    }

    #[inline]
    pub const fn as_ptr(&mut self) -> *const u8 {
        self.ptr.as_ptr()
    }
}

#[derive(Clone, Copy)]
pub struct PtrMut<'a> {
    ptr: NonNull<u8>,
    _m: PhantomData<(&'a mut u8, u8)>,
}

impl<'a> PtrMut<'a> {
    #[inline]
    pub const fn new(ptr: NonNull<u8>) -> Self {
        Self {
            ptr,
            _m: PhantomData,
        }
    }

    /// # Safety
    /// must point to type T
    #[inline]
    pub const unsafe fn cast_ref<T>(&self) -> &'a T {
        unsafe { self.ptr.cast::<T>().as_ref() }
    }

    /// # Safety
    /// must point to type T
    #[inline]
    pub const unsafe fn cast_mut<T>(&mut self) -> &'a mut T {
        unsafe { self.ptr.cast::<T>().as_mut() }
    }

    #[inline]
    pub const fn as_ptr(&mut self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    #[inline]
    pub const fn as_ref<'b>(&self) -> Ptr<'b> {
        Ptr {
            ptr: self.ptr,
            _m: PhantomData
        }
    }
}
