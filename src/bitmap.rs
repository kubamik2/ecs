use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign};

use crate::MAX_COMPONENTS;

#[derive(Default, Copy, Clone, Hash, PartialEq, Eq)]
pub struct Bitmap(u32);

impl Bitmap {
    pub const fn with_set(self, index: usize) -> Self {
        assert!(index < MAX_COMPONENTS);
        Self(self.0 | (1 << index))
    }

    #[inline]
    pub const fn get(&self, index: usize) -> bool {
        assert!(index < MAX_COMPONENTS);
        let mask: u32 = 1 << index;
        (self.0 & mask) > 0
    }

    #[inline]
    pub const fn set(&mut self, index: usize) {
        assert!(index < MAX_COMPONENTS);
        let mask: u32 = 1 << index;
        self.0 |= mask;
    }

    #[inline]
    pub const fn unset(&mut self, index: usize) {
        assert!(index < MAX_COMPONENTS);
        let mask: u32 = 1 << index;
        self.0 &= !mask;
    }

    #[inline]
    pub const fn join(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn is_zero(&self) -> bool {
        self.0 == 0    
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
