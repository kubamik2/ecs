use std::fmt::Display;
use crate::bitmap::Bitmap;

#[derive(Default, Clone)]
pub struct Access {
    resource_immutable: Bitmap,
    resource_mutable: Bitmap,
    component_immutable: Bitmap,
    component_mutable: Bitmap,
}

impl Access {
    pub fn conflicts(&self, other: &Self) -> bool {
        !( (self.resource_immutable & other.resource_mutable).is_zero() &&
            (other.resource_immutable & self.resource_mutable).is_zero() &&
            (self.resource_mutable & other.resource_mutable).is_zero() &&
            (self.component_immutable & other.component_mutable).is_zero() &&
            (other.component_immutable & self.component_mutable).is_zero() &&
            (self.component_mutable & other.component_mutable).is_zero() 
        )
    }

    pub fn is_compatible(&self, other: &Self) -> bool {
        (self.resource_immutable & other.resource_mutable).is_zero() &&
        (other.resource_immutable & self.resource_mutable).is_zero() &&
        (self.resource_mutable & other.resource_mutable).is_zero() &&
        (self.component_immutable & other.component_mutable).is_zero() &&
        (other.component_immutable & self.component_mutable).is_zero() &&
        (self.component_mutable & other.component_mutable).is_zero() 
    }

    // Both Accesses must be compatible with each other
    pub fn join(&mut self, other: &Self) {
        debug_assert!(!self.conflicts(other));
        self.resource_immutable |= other.resource_immutable;
        self.resource_mutable |= other.resource_mutable;
        self.component_immutable |= other.component_immutable;
        self.component_mutable |= other.component_mutable;
    }

    pub fn resource_immutable(&self) -> &Bitmap {
        &self.resource_immutable
    }

    pub fn resource_mutable(&self) -> &Bitmap {
        &self.resource_mutable
    }

    pub fn component_immutable(&self) -> &Bitmap {
        &self.component_immutable
    }

    pub fn component_mutable(&self) -> &Bitmap {
        &self.component_mutable
    }

    pub fn clear(&mut self) {
        self.resource_immutable = Bitmap::new();
        self.resource_mutable = Bitmap::new();
        self.component_immutable = Bitmap::new();
        self.component_mutable = Bitmap::new();
    }
}

#[derive(Default, Clone)]
pub struct AccessBuilder {
    resource_immutable: Bitmap,
    resource_mutable: Bitmap,
    component: Vec<FilteredComponentAccess>,
}

impl AccessBuilder {
    pub fn conflicts_with_component_access(&self, other: &FilteredComponentAccess) -> bool {
        self.component.iter().any(|access| access.conflicts(other))
    }

    pub fn add_resource_immutable(&mut self, index: usize) -> Result<(), Conflict> {
        if !(self.resource_mutable & Bitmap::new().with_set(index)).is_zero() {
            return Err(Conflict::ResMutImmut);
        }
        self.resource_immutable.set(index);
        Ok(())
    }

    pub fn add_resource_mutable(&mut self, index: usize) -> Result<(), Conflict> {
        let access = Bitmap::new().with_set(index);
        if !(self.resource_mutable & access).is_zero() {
            return Err(Conflict::ResDuplicateMut);
        }
        if !(self.resource_immutable & access).is_zero() {
            return Err(Conflict::ResMutImmut);
        }
        self.resource_mutable.set(index);
        Ok(())
    }

    pub fn join_filtered_component_access(&mut self, access: FilteredComponentAccess) -> Result<(), Conflict> {
        for comp_access in self.component.iter() {
            if let Some(conflict) = comp_access.get_conflict(&access) {
                return Err(conflict);
            }
        }
        self.component.push(access);
        Ok(())
    }

    pub fn build(self) -> Access {
        let mut component_immutable = Bitmap::new();
        let mut component_mutable = Bitmap::new();

        for access in self.component {
            component_immutable |= access.immutable;
            component_mutable |= access.mutable;
        }

        component_immutable &= !component_mutable;

        Access {
            resource_immutable: self.resource_immutable,
            resource_mutable: self.resource_mutable,
            component_immutable,
            component_mutable
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct FilteredComponentAccess {
    immutable: Bitmap,
    mutable: Bitmap,
    with: Bitmap,
    without: Bitmap,
}

impl FilteredComponentAccess {
    #[inline]
    pub fn is_disjoint(&self, other: &Self) -> bool {
        !((self.with & other.without).is_zero() && (other.with & other.without).is_zero())
    }

    #[inline]
    pub fn intersects(&self, other: &Self) -> bool {
        (self.with & other.without).is_zero() && (other.with & other.without).is_zero()
    }

    #[inline]
    pub fn conflicts(&self, other: &Self) -> bool {
        self.intersects(other)
        && !(
            (self.immutable & other.mutable).is_zero() &&
            (other.immutable & self.mutable).is_zero() &&
            (self.mutable & other.mutable).is_zero()
        )
    }

    pub fn get_conflict(&self, other: &Self) -> Option<Conflict> {
        if self.is_disjoint(other) {
            return None;
        }
        if !((self.immutable & other.mutable).is_zero() && (other.immutable & self.mutable).is_zero()) {
            return Some(Conflict::CompMutImmut);
        }
        if !(self.mutable & other.mutable).is_zero() {
            return Some(Conflict::CompDuplicateMut);
        }
        None
    }

    pub fn add_immutable(&mut self, index: usize) -> Result<(), Conflict> {
        if !(self.mutable & Bitmap::new().with_set(index)).is_zero() {
            return Err(Conflict::CompMutImmut);
        }
        self.immutable.set(index);
        Ok(())
    }

    pub fn add_mutable(&mut self, index: usize) -> Result<(), Conflict> {
        let bitmap = Bitmap::new().with_set(index);
        if !(self.immutable & bitmap).is_zero() {
            return Err(Conflict::CompMutImmut);
        }
        if !(self.mutable & bitmap).is_zero() {
            return Err(Conflict::CompDuplicateMut);
        }
        self.mutable.set(index);
        Ok(())
    }

    pub fn add_with(&mut self, index: usize) -> Result<(), Conflict> {
        let bitmap = Bitmap::new().with_set(index);
        if !(self.without & bitmap).is_zero() {
            return Err(Conflict::CompEmptySet);
        }
        self.with.set(index);
        Ok(())
    }

    pub fn join_with(&mut self, bitmap: Bitmap) -> Result<(), Conflict> {
        let sum = self.with | bitmap;
        if !(self.without & sum).is_zero() {
            return Err(Conflict::CompEmptySet);
        }
        self.with = sum;
        Ok(())
    }

    pub fn add_without(&mut self, index: usize) -> Result<(), Conflict> {
        let bitmap = Bitmap::new().with_set(index);
        if !((self.with & bitmap).is_zero() && ((self.immutable | self.mutable) & bitmap).is_zero()) {
            return Err(Conflict::CompEmptySet);
        }
        self.without.set(index);
        Ok(())
    }

    pub fn join_without(&mut self, bitmap: Bitmap) -> Result<(), Conflict> {
        let sum = self.without | bitmap;
        if !((self.with & sum).is_zero() && ((self.immutable | self.mutable) & sum).is_zero()) {
            return Err(Conflict::CompEmptySet);
        }
        self.without = sum;
        Ok(())
    }

    pub fn immutable(&self) -> &Bitmap {
        &self.immutable
    }

    pub fn mutable(&self) -> &Bitmap {
        &self.mutable
    }

    pub fn with(&self) -> &Bitmap {
        &self.with
    }

    pub fn without(&self) -> &Bitmap {
        &self.without
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Conflict {
    ResDuplicateMut,
    ResMutImmut,
    CompDuplicateMut,
    CompMutImmut,
    CompEmptySet
}

impl Display for Conflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ResMutImmut => f.write_str("resource mutable and immutable reference to the same value"),
            Self::ResDuplicateMut => f.write_str("resource duplicate mutable references to the same value"),
            Self::CompMutImmut => f.write_str("component mutable and immutable reference to the same value"),
            Self::CompDuplicateMut => f.write_str("component duplicate mutable references to the same value"),
            Self::CompEmptySet => f.write_str("filters produce an empty set"),
        }
    }
}

