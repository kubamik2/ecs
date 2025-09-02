use std::{any::{Any, TypeId}, cell::SyncUnsafeCell, collections::HashMap, ops::{Deref, DerefMut}};

use crate::{storage::sparse_set::SparseSet, system::SystemHandle, world::WorldPtr};

use super::{access::Access, param::SystemParam, Resource, World};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ResourceId(usize);

impl ResourceId {
    #[inline]
    pub const fn get(&self) -> usize {
        self.0
    }
}

#[derive(Default)]
pub struct Resources {
    ids: HashMap<TypeId, ResourceId>,
    sparse_set: SparseSet<SyncUnsafeCell<Box<dyn Any>>>,
}

unsafe impl Sync for Resources {}
unsafe impl Send for Resources {}

impl Resources {
    #[inline]
    fn initialize_resource<R: Resource>(resource: R) -> SyncUnsafeCell<Box<dyn Any>> {
        SyncUnsafeCell::new(Box::new(resource))
    }

    #[inline]
    fn deinitialize_resource<R: Resource>(initialized_resource: SyncUnsafeCell<Box<dyn Any>>) -> R {
        *(initialized_resource.into_inner().downcast::<R>().expect("Resources::remove invalid cast"))
    }

    pub fn get<R: Resource>(&self) -> Option<Res<R>> {
        let id = *self.ids.get(&TypeId::of::<R>())?;
        let raw = self.sparse_set.get(id.get())?;
        let val = unsafe { raw.get().as_ref().unwrap_unchecked().downcast_ref_unchecked::<R>() };
        Some(Res { val })
    }

    pub fn get_mut<R: Resource>(&mut self) -> Option<ResMut<R>> {
        let id = *self.ids.get(&TypeId::of::<R>())?;
        let raw = self.sparse_set.get(id.get())?;
        let val = unsafe { raw.get().as_mut().unwrap_unchecked().downcast_mut_unchecked::<R>() };
        Some(ResMut { val })
    }

    pub fn insert<R: Resource>(&mut self, resource: R) -> Option<R> {
        let ids_len = self.ids.len();
        match self.ids.entry(TypeId::of::<R>()) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                let id = ids_len;
                entry.insert(ResourceId(id));
                self.sparse_set.insert(id, Self::initialize_resource(resource)).map(Self::deinitialize_resource)
            },
            std::collections::hash_map::Entry::Occupied(entry) => {
                let id = *entry.get();
                self.sparse_set.insert(id.get(), Self::initialize_resource(resource)).map(Self::deinitialize_resource)
            },
        }
    }

    pub fn remove<R: Resource>(&mut self) -> Option<R> {
        let id = *self.ids.get(&TypeId::of::<R>())?;
        self.sparse_set.remove(id.get()) .map(Self::deinitialize_resource)
    }

    pub fn get_or_insert<R: Resource>(&mut self, default: R) -> ResMut<'_, R> {
        let ids_len = self.ids.len();
        let val = match self.ids.entry(TypeId::of::<R>()) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                let id = ids_len;
                entry.insert(ResourceId(id));
                self.sparse_set.insert(id, Self::initialize_resource(default));
                unsafe { self.sparse_set.get_mut(id).expect("Resources::get_or_insert inserted resource not present")
                                    .get_mut().downcast_mut_unchecked::<R>() }
            },
            std::collections::hash_map::Entry::Occupied(entry) => {
                let id = entry.get().get();
                unsafe { self.sparse_set.entry(id).or_insert_with(|| Self::initialize_resource(default)).get_mut().downcast_mut_unchecked::<R>() }
            }
        };
        ResMut { val }
    }

    pub fn get_or_insert_with<R: Resource, F: FnOnce() -> R>(&mut self, f: F) -> ResMut<'_, R> {
        let ids_len = self.ids.len();
        let val = match self.ids.entry(TypeId::of::<R>()) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                let id = ids_len;
                entry.insert(ResourceId(id));
                self.sparse_set.insert(id, Self::initialize_resource(f()));
                unsafe { self.sparse_set.get_mut(id).expect("Resources::get_or_insert inserted resource not present")
                                    .get_mut().downcast_mut_unchecked::<R>() }
            },
            std::collections::hash_map::Entry::Occupied(entry) => {
                let id = entry.get().get();
                unsafe { self.sparse_set.entry(id).or_insert_with(|| Self::initialize_resource(f())).get_mut().downcast_mut_unchecked::<R>() }
            }
        };
        ResMut { val }
    }

    #[inline]
    pub fn get_resource_id<R: Resource>(&self) -> Option<ResourceId> {
        self.ids.get(&TypeId::of::<R>()).copied()
    }

    #[inline]
    pub unsafe fn get_resource_by_id<R: Resource>(&self, id: ResourceId) -> Option<Res<'_, R>> {
        self.sparse_set.get(id.get()).map(|raw| {
            let val = unsafe { raw.get().as_ref().unwrap_unchecked().downcast_ref::<R>().expect("Resources::get_resource_by_id invalid cast") };
            Res { val }
        })
    }

    #[inline]
    pub unsafe fn get_mut_resource_by_id<R: Resource>(&self, id: ResourceId) -> Option<ResMut<'_, R>> {
        self.sparse_set.get(id.get()).map(|raw| {
            let val = unsafe { raw.get().as_mut().unwrap_unchecked().downcast_mut::<R>().expect("Resources::get_resource_by_id invalid cast") };
            ResMut { val }
        })
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
    type State = ResourceId;

    fn join_resource_access(world: &mut World, resource_access: &mut Access) {
        resource_access.immutable.set(world.resource_id::<R>().get());
    }

    fn init_state(world: &mut World) -> Self::State {
        world.resource_id::<R>()
    }

    unsafe fn fetch<'a>(world_ptr: WorldPtr<'a>, state: &'a mut Self::State, _: &SystemHandle) -> Self::Item<'a> {
        unsafe { world_ptr.as_world().resource_by_id::<R>(*state) }
    }
}

impl<R: Resource + Send + Sync + 'static> SystemParam for ResMut<'_, R> {
    type Item<'b> = ResMut<'b, R>;
    type State = ResourceId;

    fn join_resource_access(world: &mut World, resource_access: &mut Access) {
        resource_access.mutable.set(world.resource_id::<R>().get());
        resource_access.mutable_count += 1;
    }

    fn init_state(world: &mut World) -> Self::State {
        world.resource_id::<R>()
    }

    unsafe fn fetch<'a>(mut world_ptr: WorldPtr<'a>, state: &'a mut Self::State, _: &SystemHandle) -> Self::Item<'a> {
        unsafe { world_ptr.as_world_mut().resource_mut_by_id(*state) }
    }
}
