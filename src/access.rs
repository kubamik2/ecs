use std::{any::TypeId, collections::HashSet};

#[derive(Default, Clone)]
pub struct Access {
    pub immutable: HashSet<TypeId>,
    pub mutable: HashSet<TypeId>,
    pub mutable_count: u32,
}

impl Access {
    pub fn is_compatible(&self, other: &Self) -> bool {
        self.immutable.intersection(&other.mutable).next().is_none() &&
        self.mutable.intersection(&other.immutable).next().is_none() &&
        self.mutable.intersection(&other.mutable).next().is_none()
    }

    pub fn join(&mut self, other: &Self) {
        self.immutable.extend(other.immutable.iter());
        self.mutable.extend(other.mutable.iter());
        self.mutable_count += other.mutable_count;
    }
}

#[derive(Default, Clone)]
pub struct SignalAccess {
    pub required: HashSet<TypeId>,
    pub optional: HashSet<TypeId>,
}
