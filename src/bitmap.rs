use std::{fmt::{Debug, Display}, ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Deref}};

use super::MAX_COMPONENTS;

type StorageType = u128;

#[derive(Default, Copy, Clone, Hash, PartialEq, Eq)]
pub struct Bitmap(StorageType);

#[allow(unused)]
impl Bitmap {
    #[inline]
    pub const fn new() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn with_set(self, index: usize) -> Self {
        assert!(index < MAX_COMPONENTS);
        Self(self.0 | (1 << index))
    }

    #[inline]
    pub const fn get(&self, index: usize) -> bool {
        assert!(index < MAX_COMPONENTS);
        let mask: StorageType = 1 << index;
        (self.0 & mask) > 0
    }

    #[inline]
    pub const fn set(&mut self, index: usize) {
        assert!(index < MAX_COMPONENTS);
        let mask: StorageType = 1 << index;
        self.0 |= mask;
    }

    #[inline]
    pub const fn unset(&mut self, index: usize) {
        assert!(index < MAX_COMPONENTS);
        let mask: StorageType = 1 << index;
        self.0 &= !mask;
    }

    #[inline]
    pub const fn join(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn is_zero(&self) -> bool {
        self.0 == 0    
    }

    pub const fn ones(&self) -> u32 {
        self.0.count_ones()
    }
}

impl Debug for Bitmap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Bitmap").field(&format_args!("{:0>32b}", self.0)).finish()
    }
}

impl Display for Bitmap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:0>32b}", self.0))
    }
}

impl BitAnd for Bitmap {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for Bitmap {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0
    }
}

impl BitOr for Bitmap {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for Bitmap {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0
    }
}

impl Deref for Bitmap {
    type Target = StorageType;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
