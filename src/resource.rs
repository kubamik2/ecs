use std::{any::{Any, TypeId}, cell::SyncUnsafeCell, collections::HashMap, marker::PhantomData, ops::{Deref, DerefMut}};

use crate::{Commands, storage::sparse_set::SparseSet, system::SystemHandle, world::WorldPtr};

use super::{access::Access, param::SystemParam, World};

pub trait Resource: Send + Sync + 'static {
    fn on_add(&mut self, commands: &mut Commands) {}
    fn on_remove(&mut self, commands: &mut Commands) {}
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ResourceId(usize);

impl ResourceId {
    #[inline]
    pub const fn get(&self) -> usize {
        self.0
    } }

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

    pub fn get<'a, R: Resource>(&self) -> Option<&'a R> {
        let id = *self.ids.get(&TypeId::of::<R>())?;
        let raw = self.sparse_set.get(id.get())?;
        let val = unsafe { raw.get().as_ref().unwrap_unchecked().downcast_ref_unchecked::<R>() };
        Some(val)
    }

    pub fn get_mut<'a, R: Resource>(&mut self) -> Option<&'a mut R> {
        let id = *self.ids.get(&TypeId::of::<R>())?;
        let raw = self.sparse_set.get(id.get())?;
        let val = unsafe { raw.get().as_mut().unwrap_unchecked().downcast_mut_unchecked::<R>() };
        Some(val)
    }

    pub fn register<R: Resource>(&mut self) -> ResourceId {
        let ids_len = self.ids.len();
        match self.ids.entry(TypeId::of::<R>()) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                let id = ids_len;
                entry.insert(ResourceId(id));
                ResourceId(id)
            },
            std::collections::hash_map::Entry::Occupied(entry) => {
                *entry.get()
            },
        }
    }

    pub fn insert<R: Resource>(&mut self, resource: R) -> Option<R> {
        let id = self.register::<R>();
        self.sparse_set.insert(id.get(), Self::initialize_resource(resource)).map(Self::deinitialize_resource)
    }

    pub fn remove<R: Resource>(&mut self) -> Option<R> {
        let id = *self.ids.get(&TypeId::of::<R>())?;
        self.sparse_set.remove(id.get()) .map(Self::deinitialize_resource)
    }

    pub fn get_or_insert_with<'a, 'b: 'a, R: Resource, F: FnOnce() -> R>(&'b mut self, f: F) -> &'a mut R {
        let ids_len = self.ids.len();
        match self.ids.entry(TypeId::of::<R>()) {
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
        }
    }

    #[inline]
    pub fn get_resource_id<R: Resource>(&self) -> Option<ResourceId> {
        self.ids.get(&TypeId::of::<R>()).copied()
    }

    #[inline]
    pub fn get_resource_by_id<'a, R: Resource>(&self, id: ResourceId) -> Option<&'a R> {
        self.sparse_set.get(id.get()).map(|raw| {
            unsafe { raw.get().as_ref().unwrap_unchecked().downcast_ref::<R>().expect("Resources::get_resource_by_id invalid cast") }
        })
    }

    #[inline]
    pub fn get_resource_by_id_mut<'a, R: Resource>(&mut self, id: ResourceId) -> Option<&'a mut R> {
        self.sparse_set.get(id.get()).map(|raw| {
            unsafe { raw.get().as_mut().unwrap_unchecked().downcast_mut::<R>().expect("Resources::get_resource_by_id invalid cast") }
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
    val: &'a mut R,
    was_modified: &'a mut bool,
}

impl<R: Resource + Send + Sync> Deref for ResMut<'_, R> {
    type Target = R;
    fn deref(&self) -> &Self::Target {
        self.val
    }
}

impl<R: Resource + Send + Sync> DerefMut for ResMut<'_, R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        *self.was_modified = true;
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
        let val = unsafe { world_ptr.as_world().resource_by_id::<R>(*state) };
        Res { val }
    }
}

impl<R: Resource + Send + Sync + 'static> SystemParam for ResMut<'_, R> {
    type Item<'b> = ResMut<'b, R>;
    type State = (ResourceId, bool);
    //                          ^ was_modified

    fn join_resource_access(world: &mut World, resource_access: &mut Access) {
        resource_access.mutable.set(world.resource_id::<R>().get());
        resource_access.mutable_count += 1;
    }

    fn init_state(world: &mut World) -> Self::State {
        (world.resource_id::<R>(), false)
    }

    unsafe fn fetch<'a>(mut world_ptr: WorldPtr<'a>, state: &'a mut Self::State, _: &SystemHandle) -> Self::Item<'a> {
        let val = unsafe { world_ptr.as_world_mut().resource_by_id_mut::<R>(state.0) };
        ResMut { val, was_modified: &mut state.1 }
    }

    fn after(commands: &mut Commands, state: &mut Self::State) {
        if state.1 {
            commands.send_event(Changed::<R>(PhantomData));
            commands.send_signal(Changed::<R>(PhantomData), None);
            state.1 = false;
        }
    }
}

impl<R: Resource + Send + Sync + 'static> SystemParam for Option<Res<'_, R>> {
    type Item<'b> = Option<Res<'b, R>>;
    type State = ResourceId;

    fn join_resource_access(world: &mut World, resource_access: &mut Access) {
        resource_access.immutable.set(world.resource_id::<R>().get());
    }

    fn init_state(world: &mut World) -> Self::State {
        world.register_resource::<R>()
    }

    unsafe fn fetch<'a>(world_ptr: WorldPtr<'a>, state: &'a mut Self::State, _: &SystemHandle) -> Self::Item<'a> {
        let val = unsafe { world_ptr.as_world().get_resource_by_id::<R>(*state) }?;
        Some(Res { val })
    }
}

impl<R: Resource + Send + Sync + 'static> SystemParam for Option<ResMut<'_, R>> {
    type Item<'b> = Option<ResMut<'b, R>>;
    type State = (ResourceId, bool);
    //                          ^ was_modified

    fn join_resource_access(world: &mut World, resource_access: &mut Access) {
        resource_access.mutable.set(world.resource_id::<R>().get());
        resource_access.mutable_count += 1;
    }

    fn init_state(world: &mut World) -> Self::State {
        (world.register_resource::<R>(), false)
    }

    unsafe fn fetch<'a>(mut world_ptr: WorldPtr<'a>, state: &'a mut Self::State, _: &SystemHandle) -> Self::Item<'a> {
        let val = unsafe { world_ptr.as_world_mut().get_resource_by_id_mut::<R>(state.0) }?;
        Some(ResMut { val, was_modified: &mut state.1 })
    }

    fn after(commands: &mut Commands, state: &mut Self::State) {
        if state.1 {
            commands.send_event(Changed::<R>(PhantomData));
            commands.send_signal(Changed::<R>(PhantomData), None);
            state.1 = false;
        }
    }
}

#[derive(Default)]
pub struct Changed<T>(PhantomData<T>);

impl<T> Changed<T> {
    #[inline]
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}
