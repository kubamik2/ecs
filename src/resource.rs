use std::{any::{Any, TypeId}, cell::SyncUnsafeCell, collections::HashMap, ops::{Deref, DerefMut}};

use crate::sparse_set::SparseSet;

use super::{access::Access, param::SystemParam, Resource, World};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ResourceId(u16);

impl Deref for ResourceId {
    type Target = u16;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Default)]
pub struct Resources {
    ids: HashMap<TypeId, ResourceId>,
    map: HashMap<TypeId, SyncUnsafeCell<Box<dyn Any>>>,
    sparse_set: SparseSet<SyncUnsafeCell<Box<dyn Any>>>,
}

unsafe impl Sync for Resources {}
unsafe impl Send for Resources {}

impl Resources {
    fn initialize_resource<R: Resource>(resource: R) -> SyncUnsafeCell<Box<dyn Any>> {
        SyncUnsafeCell::new(Box::new(resource))
    }

    #[inline]
    fn deinitialize_resource<R: Resource>(initialized_resource: SyncUnsafeCell<Box<dyn Any>>) -> R {
        *(initialized_resource.into_inner().downcast::<R>().expect("Resources::remove invalid cast"))
    }

    pub fn get<R: Resource>(&self) -> Option<Res<R>> {
        let id = *self.ids.get(&TypeId::of::<R>())?;
        let raw = self.sparse_set.get(*id)?;
        let val = unsafe { raw.get().as_ref().unwrap_unchecked().downcast_ref_unchecked::<R>() };
        Some(Res { val })
    }

    pub fn get_mut<R: Resource>(&mut self) -> Option<ResMut<R>> {
        let id = *self.ids.get(&TypeId::of::<R>())?;
        let raw = self.sparse_set.get(*id)?;
        let val = unsafe { raw.get().as_mut().unwrap_unchecked().downcast_mut_unchecked::<R>() };
        Some(ResMut { val })
    }

    pub unsafe fn get_mut_unsafe<R: Resource>(&self) -> Option<ResMut<R>> {
        let id = *self.ids.get(&TypeId::of::<R>())?;
        let raw = self.sparse_set.get(*id)?;
        let val = unsafe { raw.get().as_mut().unwrap_unchecked().downcast_mut_unchecked::<R>() };
        Some(ResMut { val })
    }

    pub fn insert<R: Resource>(&mut self, resource: R) -> Option<R> {
        let ids_len = self.ids.len();
        match self.ids.entry(TypeId::of::<R>()) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                assert!(ids_len <= crate::sparse_set::SPARSE_SET_CAPACITY);
                let id = ids_len as u16;
                entry.insert(ResourceId(id));
                self.sparse_set.insert(id, Self::initialize_resource(resource)).map(Self::deinitialize_resource)
            },
            std::collections::hash_map::Entry::Occupied(entry) => {
                let id = *entry.get();
                self.sparse_set.insert(*id, Self::initialize_resource(resource)).map(Self::deinitialize_resource)
            },
        }
    }

    pub fn remove<R: Resource>(&mut self) -> Option<R> {
        let id = *self.ids.get(&TypeId::of::<R>())?;
        self.sparse_set.remove(*id) .map(Self::deinitialize_resource)
    }

    pub fn get_or_insert<'a, R: Resource>(&'a mut self, default: R) -> ResMut<'a, R> {
        let ids_len = self.ids.len();
        let val = match self.ids.entry(TypeId::of::<R>()) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                assert!(ids_len <= crate::sparse_set::SPARSE_SET_CAPACITY);
                let id = ids_len as u16;
                entry.insert(ResourceId(id));
                self.sparse_set.insert(id, Self::initialize_resource(default));
                unsafe { self.sparse_set.get_mut(id).expect("Resources::get_or_insert inserted resource not present")
                                    .get_mut().downcast_mut_unchecked::<R>() }
            },
            std::collections::hash_map::Entry::Occupied(entry) => {
                let id = **entry.get();
                unsafe { self.sparse_set.entry(id).or_insert_with(|| Self::initialize_resource(default)).get_mut().downcast_mut_unchecked::<R>() }
            }
        };
        ResMut { val }
    }
}

pub struct Res<'a, R: Resource + Send + Sync> {
    val: &'a R
}

impl<R: Resource + Send + Sync> Deref for Res<'_, R> {
    type Target = R;
    fn deref(&self) -> &Self::Target {
        self.val
    }
}

pub struct ResMut<'a, R: Resource> {
    val: &'a mut R
}

impl<R: Resource + Send + Sync> Deref for ResMut<'_, R> {
    type Target = R;
    fn deref(&self) -> &Self::Target {
        self.val
    }
}

impl<R: Resource + Send + Sync> DerefMut for ResMut<'_, R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.val
    }
}

impl<R: Resource + Send + Sync + 'static> SystemParam for Res<'_, R> {
    type Item<'b> = Res<'b, R>;
    type State = ();

    fn join_resource_access(resource_access: &mut Access) {
        resource_access.immutable.insert(TypeId::of::<R>());
    }

    fn init_state(_: &mut World) -> Self::State {}

    fn fetch<'a>(world: &'a World, _: &'a mut Self::State) -> Self::Item<'a> {
        world.resource::<R>()
    }
}

impl<R: Resource + Send + Sync + 'static> SystemParam for ResMut<'_, R> {
    type Item<'b> = ResMut<'b, R>;
    type State = ();

    fn join_resource_access(resource_access: &mut Access) {
        resource_access.mutable.insert(TypeId::of::<R>());
        resource_access.mutable_count += 1;
    }

    fn init_state(_: &mut World) -> Self::State {
    }

    fn fetch<'a>(world: &'a World, _: &'a mut Self::State) -> Self::Item<'a> {
        unsafe { world.resource_mut_unsafe::<R>() }
    }
}
