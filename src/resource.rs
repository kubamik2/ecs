use std::{any::Any, cell::SyncUnsafeCell, ops::{Deref, DerefMut}, ptr::NonNull};

use crate::{access::Access, bitmap::Bitmap, param::SystemParam, Resource, MAX_RESOURCES};

#[derive(Default)]
pub struct ResourceManager {
    resources: [Option<SyncUnsafeCell<Box<dyn Any>>>; MAX_RESOURCES],
}
unsafe impl Sync for ResourceManager {}
unsafe impl Send for ResourceManager {}

impl ResourceManager {
    pub fn get<R: Resource + Send + Sync + 'static>(&self) -> Option<Res<R>> {
        let boxed_any = self.resources[R::signature_index()].as_ref()?;
        Some(Res { val: SyncPointer(NonNull::from(unsafe { boxed_any.get().as_mut().unwrap_unchecked().downcast_mut::<R>().unwrap_unchecked() })) })
    }

    pub fn get_mut<R: Resource + Send + Sync + 'static>(&self) -> Option<ResMut<R>> {
        let boxed_any = self.resources[R::signature_index()].as_ref()?;
        Some(ResMut { val: SyncPointer(NonNull::from(unsafe { boxed_any.get().as_mut().unwrap_unchecked().downcast_mut::<R>().unwrap_unchecked() })) })
    }

    pub fn set<R: Resource + Send + Sync + 'static>(&mut self, resource: R) {
        self.resources[R::signature_index()] = Some(SyncUnsafeCell::new(Box::new(resource)));
    }

    pub fn remove<R: Resource + Send + Sync + 'static>(&mut self) -> Option<Box<R>> {
        let val = self.resources[R::signature_index()].take()?;
        let boxed_any = val.into_inner();
        boxed_any.downcast::<R>().ok()
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
    fn create(_: &crate::component_manager::ComponentManager, resource_manager: &ResourceManager) -> Option<Self> {
        resource_manager.get::<R>()
    }
    fn resource_access() -> Access {
        Access {
            immutable: Bitmap::new().with_set(R::signature_index()),
            ..Default::default()
        }
    }
}

impl<R: Resource + Send + Sync + 'static> SystemParam for ResMut<R> {
    fn create(_: &crate::component_manager::ComponentManager, resource_manager: &ResourceManager) -> Option<Self> {
        resource_manager.get_mut::<R>()
    }

    fn resource_access() -> Access {
        Access {
            mutable: Bitmap::new().with_set(R::signature_index()),
            mutable_count: 1,
            ..Default::default()
        }
    }
}
