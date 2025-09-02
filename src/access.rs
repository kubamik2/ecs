use crate::bitmap::Bitmap;

#[derive(Default, Clone)]
pub struct Access {
    pub immutable: Bitmap,
    pub mutable: Bitmap,
    pub mutable_count: u32,
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
}
