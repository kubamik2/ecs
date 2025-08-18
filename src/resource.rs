use std::{any::{Any, TypeId}, cell::SyncUnsafeCell, collections::HashMap, ops::{Deref, DerefMut}, ptr::NonNull};

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
        Some(Res { val: SyncPointer(NonNull::from(unsafe { boxed_any.get().as_mut().unwrap_unchecked().downcast_mut::<R>().unwrap_unchecked() })) })
    }

    pub fn get_mut<R: Resource + Send + Sync + 'static>(&self) -> Option<ResMut<R>> {
        let boxed_any = self.resources.get(&TypeId::of::<R>())?;
        Some(ResMut { val: SyncPointer(NonNull::from(unsafe { boxed_any.get().as_mut().unwrap_unchecked().downcast_mut::<R>().unwrap_unchecked() })) })
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

pub struct SyncPointer<T>(NonNull<T>);
unsafe impl<T> Sync for SyncPointer<T> {}
unsafe impl<T> Send for SyncPointer<T> {}

pub struct Res<R: Resource + Send + Sync> {
    val: SyncPointer<R>,
}

impl<R: Resource + Send + Sync> Deref for Res<R> {
    type Target = R;
    fn deref(&self) -> &Self::Target {
        unsafe { self.val.0.as_ref() }
    }
}

pub struct ResMut<R: Resource> {
    val: SyncPointer<R>,
}

impl<R: Resource + Send + Sync> Deref for ResMut<R> {
    type Target = R;
    fn deref(&self) -> &Self::Target {
        unsafe { self.val.0.as_ref() }
    }
}

impl<R: Resource + Send + Sync> DerefMut for ResMut<R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.val.0.as_mut() }
    }
}

impl<R: Resource + Send + Sync + 'static> SystemParam for Res<R> {
    fn create(ecs: &ECS) -> Option<Self> {
        ecs.resource_manager.get::<R>()
    }
    fn join_resource_access(resource_access: &mut Access) {
        resource_access.immutable.insert(TypeId::of::<R>());
    }
}

impl<R: Resource + Send + Sync + 'static> SystemParam for ResMut<R> {
    fn create(ecs: &ECS) -> Option<Self> {
        ecs.resource_manager.get_mut::<R>()
    }

    fn join_resource_access(resource_access: &mut Access) {
        resource_access.mutable.insert(TypeId::of::<R>());
        resource_access.mutable_count += 1;
    }
}
