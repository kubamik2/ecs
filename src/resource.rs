use std::{any::{Any, TypeId}, cell::SyncUnsafeCell, collections::HashMap, ops::{Deref, DerefMut}};

use super::{access::Access, param::SystemParam, Resource, ECS};

#[derive(Default)]
pub struct Resources {
    map: HashMap<TypeId, SyncUnsafeCell<Box<dyn Any>>>
}

unsafe impl Sync for Resources {}
unsafe impl Send for Resources {}

impl Resources {
    pub fn get<R: Resource>(&self) -> Option<Res<R>> {
        let boxed_any = self.map.get(&TypeId::of::<R>())?;
        let val = unsafe { boxed_any.get().as_ref().unwrap_unchecked().downcast_ref_unchecked::<R>() };
        Some(Res { val })
    }

    pub fn get_mut<R: Resource>(&mut self) -> Option<ResMut<R>> {
        let boxed_any = self.map.get(&TypeId::of::<R>())?;
        let val = unsafe { boxed_any.get().as_mut().unwrap_unchecked().downcast_mut_unchecked::<R>() };
        Some(ResMut { val })
    }

    pub unsafe fn get_mut_unsafe<R: Resource>(&self) -> Option<ResMut<R>> {
        let boxed_any = self.map.get(&TypeId::of::<R>())?;
        let val = unsafe { boxed_any.get().as_mut().unwrap_unchecked().downcast_mut_unchecked::<R>() };
        Some(ResMut { val })
    }

    pub fn insert<R: Resource>(&mut self, resource: R) {
        self.map.insert(TypeId::of::<R>(), SyncUnsafeCell::new(Box::new(resource)));
    }

    pub fn remove<R: Resource>(&mut self) -> Option<R> {
        self.map
            .remove(&TypeId::of::<R>())
            .map(|f| *f.into_inner().downcast::<R>().expect("ResourceManager::remove invalid cast"))
    }

    pub fn get_or_insert_default<R: Resource + Default>(&mut self) -> &R {
        let boxed_any = self.map.entry(TypeId::of::<R>()).or_insert(SyncUnsafeCell::new(Box::new(R::default())));
        unsafe { boxed_any.get().as_ref().unwrap_unchecked().downcast_ref_unchecked::<R>() }
    }

    pub fn get_mut_or_insert_default<R: Resource + Default>(&mut self) -> &mut R {
        let boxed_any = self.map.entry(TypeId::of::<R>()).or_insert(SyncUnsafeCell::new(Box::new(R::default())));
        unsafe { boxed_any.get().as_mut().unwrap_unchecked().downcast_mut_unchecked::<R>() }
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

    fn init_state(_: &mut ECS) -> Self::State {}

    fn fetch<'a>(ecs: &'a ECS, _: &'a mut Self::State) -> Self::Item<'a> {
        ecs.resources.get::<R>()
            .unwrap_or_else(|| panic!("resource '{}' is not initialized", std::any::type_name::<R>()))
    }
}

impl<R: Resource + Send + Sync + 'static> SystemParam for ResMut<'_, R> {
    type Item<'b> = ResMut<'b, R>;
    type State = ();

    fn join_resource_access(resource_access: &mut Access) {
        resource_access.mutable.insert(TypeId::of::<R>());
        resource_access.mutable_count += 1;
    }

    fn init_state(_: &mut ECS) -> Self::State {
    }

    fn fetch<'a>(ecs: &'a ECS, _: &'a mut Self::State) -> Self::Item<'a> {
        unsafe { ecs.resources.get_mut_unsafe::<R>()
            .unwrap_or_else(|| panic!("resource '{}' is not initialized", std::any::type_name::<R>())) }
    }
}
