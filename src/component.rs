use std::{any::TypeId, collections::{hash_map::Entry, HashMap}, mem::MaybeUninit, ops::BitOrAssign};

use crate::{bitmap::Bitmap, sparse_set::{SparseSet, TypelessSparseSet}, Component, Entity, MAX_COMPONENTS};

pub type Signature = Bitmap;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentId(usize);

impl ComponentId {
    pub fn id(&self) -> usize {
        self.0
    }

    pub(crate) fn as_signature(&self) -> Signature {
        Signature::new().with_set(self.0)
    }
}

pub struct ComponentRecord {
    signature: Signature,
    id: ComponentId,
}

pub struct Components {
    component_records: HashMap<TypeId, ComponentRecord>,
    components: [MaybeUninit<TypelessSparseSet>; MAX_COMPONENTS],
    groups: HashMap<Signature, SparseSet<Entity>>,
    entity_signatures: SparseSet<Signature>,
    component_len: usize,
}

impl Default for Components {
    fn default() -> Self {
        Self {
            component_records: Default::default(),
            components: std::array::from_fn(|_| MaybeUninit::uninit()),
            groups: Default::default(),
            entity_signatures: Default::default(),
            component_len: 0,
        }
    }
}

impl Components {
    pub(crate) fn register_component<C: Component>(&mut self) -> ComponentId {
        match self.component_records.entry(TypeId::of::<C>()) {
            Entry::Vacant(vacant) => {
                let id = ComponentId(self.component_len);
                let signature = Bitmap::new().with_set(self.component_len);
                vacant.insert(ComponentRecord {
                    signature,
                    id,
                });
                self.components[self.component_len] = MaybeUninit::new(TypelessSparseSet::new(SparseSet::<C>::new()));
                self.component_len += 1;
                id
            },
            Entry::Occupied(occupied) => {
                occupied.get().id
            },
        }
    }

    // entity must be alive
    pub(crate) fn get_component<C: Component>(&self, entity: Entity) -> Option<&C> {
        let component_id = self.get_component_id::<C>()?;
        unsafe { self.get_component_by_id::<C>(entity, component_id).map(|f| f.as_ref().unwrap_unchecked()) }
    }

    // entity must be alive
    pub(crate) fn get_mut_component<C: Component>(&mut self, entity: Entity) -> Option<&mut C> {
        let component_id = self.get_component_id::<C>()?;
        unsafe { self.get_component_by_id::<C>(entity, component_id).map(|f| f.as_mut().unwrap_unchecked()) }
    }

    // entity must be alive
    pub(crate) fn set_component<C: Component>(&mut self, entity: Entity, component: C) {
        let components_num = self.component_records.len();
        let component_record = self.component_records
            .entry(TypeId::of::<C>())
            .or_insert_with(|| {
                self.components[self.component_len] = MaybeUninit::new(TypelessSparseSet::new(SparseSet::<C>::new()));
                self.component_len += 1;
                ComponentRecord {
                    signature: Bitmap::new().with_set(components_num),
                    id: ComponentId(components_num),
                }
            });
        let Some(entity_signature) = self.entity_signatures.get_mut(entity.id()) else { return; };

        let component_signature = component_record.signature;
        let component_id = component_record.id;

        let sparse_set = unsafe { self.components[component_id.0].assume_init_ref().downcast_unchecked::<C>().get().as_mut().unwrap_unchecked() };

        if (*entity_signature & component_signature).is_zero() {
            let group = self.groups.get_mut(entity_signature).expect("entity doesnt belong to any groups");
            group.remove(entity.id());
            entity_signature.bitor_assign(component_signature);
            let new_group = self.groups.entry(*entity_signature).or_default();
            new_group.insert(entity.id(), entity);
        }
        sparse_set.insert(entity.id(), component);
    }

    // entity must be alive
    pub(crate) fn remove_component<C: Component>(&mut self, entity: Entity) {
        let Some(component_record) = self.component_records.get(&TypeId::of::<C>()) else { return; };
        let Some(entity_signature) = self.entity_signatures.get_mut(entity.id()) else { return; };

        let component_signature = component_record.signature;
        let component_id = component_record.id;

        let sparse_set = unsafe { self.components[component_id.0].assume_init_ref().downcast_unchecked::<C>().get().as_mut().unwrap_unchecked() };
        if (*entity_signature & component_signature).is_zero() {
            return;
        }
        let group = self.groups.get_mut(entity_signature).expect("entity doesnt belong to any groups");
        group.remove(entity.id());
        entity_signature.bitor_assign(component_signature);
        let new_group = self.groups.entry(*entity_signature).or_default();
        new_group.insert(entity.id(), entity);

        sparse_set.remove(entity.id());
    }

    pub(crate) unsafe fn insert_empty_entity(&mut self, entity: Entity, signature: Signature) {
        assert!(self.entity_signatures.insert(entity.id(), signature).is_none(), "component manager duplicate EntityId");
        let group = self.groups.entry(signature).or_default();
        group.insert(entity.id(), entity);
    }

    // entity must be alive
    pub(crate) unsafe fn set_component_unchecked<C: Component>(&mut self, entity: Entity, component: C) {
        let component_record = unsafe { self.component_records.get(&TypeId::of::<C>()).unwrap_unchecked() };
        let component_id = component_record.id;
        let sparse_set = unsafe { self.components[component_id.0].assume_init_ref().downcast_unchecked::<C>().get().as_mut().unwrap_unchecked() };
        sparse_set.insert(entity.id(), component);
    }

    // entity must be alive
    pub(crate) fn remove_entity(&mut self, entity: Entity) {
        let Some(entity_signature) = self.entity_signatures.remove(entity.id()) else { return; };
        let group = self.groups.get_mut(&entity_signature).expect("entity doesnt belong to any groups");
        group.remove(entity.id());
        
        let mut entity_signature_raw = *entity_signature;
        let mut index = 0;
        while entity_signature_raw > 0 {
            if (entity_signature_raw & 1) == 1 {
                unsafe { self.components[index].assume_init_mut().remove(entity.id()) };
            }
            entity_signature_raw >>= 1;
            index += 1;
        }
    }

    pub(crate) fn get_component_signature(&self, type_id: &TypeId) -> Option<Signature> {
        let record = self.component_records.get(type_id)?;
        Some(record.signature)
    }

    pub(crate) fn get_component_id<C: Component>(&self) -> Option<ComponentId> {
        self.component_records.get(&TypeId::of::<C>()).map(|f| f.id)
    }

    // entity must be alive
    pub(crate) unsafe fn get_component_by_id<C: Component>(&self, entity: Entity, component_id: ComponentId) -> Option<*mut C> {
        let sparse_set = unsafe { self.components[component_id.0].assume_init_ref().downcast_unchecked::<C>() };
        unsafe { sparse_set.get().as_mut().unwrap_unchecked().get_mut_ptr(entity.id()) }
    }

    // entity must be alive
    pub(crate) unsafe fn get_component_by_id_unchecked<C: Component>(&self, entity: Entity, component_id: ComponentId) -> *mut C {
        let sparse_set = unsafe { self.components[component_id.0].assume_init_ref().downcast_unchecked::<C>() };
        unsafe { sparse_set.get().as_mut().unwrap_unchecked().get_mut_ptr_unchecked(entity.id()) }
    }

    #[inline]
    pub(crate) fn groups(&self) -> &HashMap<Signature, SparseSet<Entity>> {
        &self.groups
    }

    // entity must be alive
    pub(crate) fn get_entity_signature_by_type_id(&self, entity: Entity) -> Option<Signature> {
        self.entity_signatures.get(entity.id()).copied()
    }
}
