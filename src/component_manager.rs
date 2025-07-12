use std::{any::{Any, TypeId}, cell::SyncUnsafeCell, collections::HashMap, mem::MaybeUninit, ops::BitOrAssign};
use crate::{bitmap::Bitmap, Component, Entity, MAX_ENTITIES};

#[derive(Default)]
pub struct ComponentManager {
    pub component_arrays: HashMap<TypeId, Box<dyn Any>>,
    pub entity_component_signatures: HashMap<Entity, Bitmap>,
}

unsafe impl Send for ComponentManager {}
unsafe impl Sync for ComponentManager {}

impl ComponentManager {
    pub fn add_entity(&mut self, entity: Entity) {
        assert!(self.entity_component_signatures.insert(entity, Bitmap::default()).is_none())
    }

    pub fn set_entity_component<T: Component + 'static>(&mut self, entity: Entity, component: T) {
        let component_array = self.component_arrays
            .entry(TypeId::of::<T>())
            .or_insert(Box::new(ComponentArray::<T>::empty()))
            .downcast_mut::<ComponentArray<T>>().unwrap();
            
        component_array.0[entity.id() as usize].write(SyncUnsafeCell::new(component));
        self.entity_component_signatures
            .entry(entity)
            .or_default()
            .bitor_assign(Bitmap::default().with_set(T::signature_index()));
    }

    pub fn get_entity_component<T: Component + 'static>(&self, entity: Entity) -> Option<*const T> {
        let entity_component_signature = self.entity_component_signatures.get(&entity)?;
        if !entity_component_signature.get(T::signature_index()) {
            return None;
        }
        let component_array = self.component_arrays.get(&TypeId::of::<T>()).map(|f| f.downcast_ref::<ComponentArray<T>>().unwrap()).unwrap();
        let ptr = unsafe { component_array.0[entity.id() as usize].assume_init_ref().get() };
        Some(ptr as *const T)
    }

    pub fn get_mut_entity_component<T: Component + 'static>(&self, entity: Entity) -> Option<*mut T> {
        let entity_component_signature = self.entity_component_signatures.get(&entity)?;
        if !entity_component_signature.get(T::signature_index()) {
            return None;
        }
        let component_array = self.component_arrays.get(&TypeId::of::<T>()).map(|f| f.downcast_ref::<ComponentArray<T>>().unwrap()).unwrap();
        let ptr = unsafe { component_array.0[entity.id() as usize].assume_init_ref().get() };
        Some(ptr)
    }

    pub fn remove_entity(&mut self, entity: Entity) {
        self.entity_component_signatures.remove(&entity);
    }
}

pub struct ComponentArray<T>(Box<[MaybeUninit<SyncUnsafeCell<T>>; MAX_ENTITIES]>);

impl<T> ComponentArray<T> {
    pub fn empty() -> Self {
        let boxed_slice = Vec::from_iter((0..MAX_ENTITIES).map(|_| MaybeUninit::<SyncUnsafeCell<T>>::uninit())).into_boxed_slice();
        let boxed_array = unsafe { Box::from_raw(Box::into_raw(boxed_slice) as *mut [MaybeUninit<SyncUnsafeCell<T>>; MAX_ENTITIES]) };
        Self(boxed_array)
    }
}
