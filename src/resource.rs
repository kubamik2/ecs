use std::{any::{Any, TypeId}, cell::SyncUnsafeCell, collections::HashMap, ops::{Deref, DerefMut}};

use super::{access::Access, param::SystemParam, Resource, ECS};

#[derive(Default)]
pub struct ResourceManager {
    resources: HashMap<TypeId, SyncUnsafeCell<Box<dyn Any>>>
}

unsafe impl Sync for ResourceManager {}
unsafe impl Send for ResourceManager {}

impl ResourceManager {
    pub fn get<R: Resource>(&self) -> Option<Res<R>> {
        let boxed_any = self.resources.get(&TypeId::of::<R>())?;
        let val = unsafe { boxed_any.get().as_ref().unwrap_unchecked().downcast_ref_unchecked::<R>() };
        Some(Res { val })
    }

    pub fn get_mut<R: Resource + Send + Sync + 'static>(&self) -> Option<ResMut<R>> {
        let boxed_any = self.resources.get(&TypeId::of::<R>())?;
        let val = unsafe { boxed_any.get().as_mut().unwrap_unchecked().downcast_mut_unchecked::<R>() };
        Some(ResMut { val })
    }

    pub fn insert<R: Resource + Send + Sync + 'static>(&mut self, resource: R) {
        self.resources.insert(TypeId::of::<R>(), SyncUnsafeCell::new(Box::new(resource)));
    }

    pub fn remove<R: Resource + Send + Sync + 'static>(&mut self) -> Option<R> {
        self.resources
            .remove(&TypeId::of::<R>())
            .map(|f| *f.into_inner().downcast::<R>().expect("ResourceManager::remove invalid cast"))
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
    fn create(ecs: &ECS) -> Option<Self::Item<'_>> {
        ecs.resource_manager.get::<R>()
    }

    fn join_resource_access(resource_access: &mut Access) {
        resource_access.immutable.insert(TypeId::of::<R>());
    }
}

impl<R: Resource + Send + Sync + 'static> SystemParam for ResMut<'_, R> {
    type Item<'b> = ResMut<'b, R>;
    fn create(ecs: &ECS) -> Option<Self::Item<'_>> {
        ecs.resource_manager.get_mut::<R>()
    }

    fn join_resource_access(resource_access: &mut Access) {
        resource_access.mutable.insert(TypeId::of::<R>());
        resource_access.mutable_count += 1;
    }
}
