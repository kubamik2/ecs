use std::fmt::Display;

use crate::bitmap::Bitmap;

#[derive(Default, Clone)]
pub struct Access {
    immutable: Bitmap,
    mutable: Bitmap,
    mutable_count: u32,
}

impl Access {
    #[inline]
    pub fn is_compatible(&self, other: &Self) -> bool {
        (self.immutable & other.mutable).is_zero() && 
        (self.mutable & other.immutable).is_zero() &&
        (self.mutable & other.mutable).is_zero()
    }

    /// Both Accesses must be compatible with each other
    #[inline]
    pub fn join(&mut self, other: &Self) {
        debug_assert!(self.is_compatible(other));
        self.immutable |= other.immutable;
        self.mutable |= other.mutable;
        self.mutable_count += other.mutable_count;
    }

    #[inline]
    pub fn clear(&mut self) {
        self.immutable = Bitmap::new();
        self.mutable = Bitmap::new();
        self.mutable_count = 0;
    }

    pub fn immutable(&self) -> &Bitmap {
        &self.immutable
    }

    pub fn mutable(&self) -> &Bitmap {
        &self.mutable
    }

    pub fn add_immutable(&mut self, index: usize) {
        self.immutable.set(index);
    }

    pub fn add_mutable(&mut self, index: usize) {
        self.mutable.set(index);
        self.mutable_count += 1;
    }

    pub fn validate(&self) -> Result<(), AccessError> {
        debug_assert!(self.mutable_count >= self.mutable.count_ones()); // sanity check
        if !(self.mutable & self.immutable).is_zero() {
            return Err(AccessError::MutImmut);
        }
        if self.mutable_count > self.mutable.count_ones() {
            return Err(AccessError::DuplicateMut);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AccessError {
    DuplicateMut,
    MutImmut
}

impl Display for AccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MutImmut => f.write_str("mutable and immutable reference to the same value"),
            Self::DuplicateMut => f.write_str("duplicate mutable references to the same value"),
        }
    }
}
