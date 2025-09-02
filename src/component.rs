use std::{any::TypeId, collections::{hash_map::Entry, HashMap}, ops::BitOrAssign};

use crate::{bitmap::Bitmap, storage::sparse_set::{blob_sparse_set::BlobSparseSet, SparseSet}, Component, Entity};

pub type Signature = Bitmap;

pub const MAX_COMPONENTS: usize = Bitmap::WIDTH;

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

#[derive(Default)]
pub struct Components {
    component_records: HashMap<TypeId, ComponentRecord>,
    components: Vec<BlobSparseSet>,
    groups: HashMap<Signature, SparseSet<Entity>>,
    entity_signatures: SparseSet<Signature>,
    component_len: usize,
}

impl Components {
    /// Register component if absent
    pub(crate) fn register_component<C: Component>(&mut self) -> ComponentId {
        match self.component_records.entry(TypeId::of::<C>()) {
            Entry::Vacant(vacant) => {
                let id = ComponentId(self.component_len);
                let signature = Bitmap::new().with_set(self.component_len);
                vacant.insert(ComponentRecord {
                    signature,
                    id,
                });
                self.components.push(BlobSparseSet::new::<C>());
                assert!(self.component_len <= MAX_COMPONENTS, "component overflow");
                self.component_len += 1;
                id
            },
            Entry::Occupied(occupied) => {
                occupied.get().id
            },
        }
    }

    /// Entity must be alive
    pub(crate) fn get_component<C: Component>(&self, entity: Entity) -> Option<&C> {
        let component_id = self.get_component_id::<C>()?;
        unsafe { self.get_component_by_id::<C>(entity, component_id) }
    }

    /// Entity must be alive
    pub(crate) fn get_mut_component<C: Component>(&mut self, entity: Entity) -> Option<&mut C> {
        let component_id = self.get_component_id::<C>()?;
        unsafe { self.get_mut_component_by_id::<C>(entity, component_id) }
    }

    /// Entity must be alive
    pub(crate) fn set_component<C: Component>(&mut self, entity: Entity, component: C) {
        let components_num = self.component_records.len();
        let component_record = self.component_records
            .entry(TypeId::of::<C>())
            .or_insert_with(|| {
                self.components.push(BlobSparseSet::new::<C>());
                self.component_len += 1;
                assert!(self.component_len <= MAX_COMPONENTS, "component overflow");
                ComponentRecord {
                    signature: Bitmap::new().with_set(components_num),
                    id: ComponentId(components_num),
                }
            });
        let Some(entity_signature) = self.entity_signatures.get_mut(entity.id() as usize) else { return; };

        let component_signature = component_record.signature;
        let component_id = component_record.id;

        let sparse_set = &mut self.components[component_id.0];

        if (*entity_signature & component_signature).is_zero() {
            let group = self.groups.get_mut(entity_signature).expect("entity doesnt belong to any groups");
            group.remove(entity.id() as usize);
            entity_signature.bitor_assign(component_signature);
            let new_group = self.groups.entry(*entity_signature).or_default();
            new_group.insert(entity.id() as usize, entity);
        }
        unsafe { sparse_set.insert(entity.id() as usize, component) };
    }

    /// Entity must be alive
    pub(crate) fn remove_component<C: Component>(&mut self, entity: Entity) {
        let Some(component_record) = self.component_records.get(&TypeId::of::<C>()) else { return; };
        let Some(entity_signature) = self.entity_signatures.get_mut(entity.id() as usize) else { return; };

        let component_signature = component_record.signature;
        let component_id = component_record.id;

        let sparse_set = &mut self.components[component_id.0];
        if (*entity_signature & component_signature).is_zero() {
            return;
        }
        let group = self.groups.get_mut(entity_signature).expect("entity doesnt belong to any groups");
        group.remove(entity.id() as usize);
        entity_signature.bitor_assign(component_signature);
        let new_group = self.groups.entry(*entity_signature).or_default();
        new_group.insert(entity.id() as usize, entity);

        sparse_set.remove(entity.id() as usize);
    }

    pub(crate) unsafe fn insert_empty_entity(&mut self, entity: Entity, signature: Signature) {
        assert!(self.entity_signatures.insert(entity.id() as usize, signature).is_none(), "component manager duplicate EntityId");
        let group = self.groups.entry(signature).or_default();
        group.insert(entity.id() as usize, entity);
    }

    /// Entity must be alive
    pub(crate) unsafe fn set_component_unchecked<C: Component>(&mut self, entity: Entity, component: C) {
        let component_record = unsafe { self.component_records.get(&TypeId::of::<C>()).unwrap_unchecked() };
        let component_id = component_record.id;
        let sparse_set = &mut self.components[component_id.0];
        unsafe { sparse_set.insert(entity.id() as usize, component) };
    }

    /// Entity must be alive
    pub(crate) fn remove_entity(&mut self, entity: Entity) {
        let Some(entity_signature) = self.entity_signatures.remove(entity.id() as usize) else { return; };
        let group = self.groups.get_mut(&entity_signature).expect("entity doesnt belong to any groups");
        group.remove(entity.id() as usize);
        
        let mut entity_signature_raw = *entity_signature;
        let mut index = 0;
        while entity_signature_raw > 0 {
            if (entity_signature_raw & 1) == 1 {
                self.components[index].remove(entity.id() as usize);
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

    /// # Safety
    /// Entity must be alive
    /// Component_id must correspond to a component array of type C
    pub(crate) unsafe fn get_component_by_id<C: Component>(&self, entity: Entity, component_id: ComponentId) -> Option<&C> {
        let sparse_set = &self.components[component_id.0];
        unsafe { sparse_set.get(entity.id() as usize) }
    }

    /// # Safety
    /// Entity must be alive
    /// Component_id must correspond to a component array of type C
    pub(crate) unsafe fn get_component_by_id_unchecked<C: Component>(&self, entity: Entity, component_id: ComponentId) -> &C {
        let sparse_set = &self.components[component_id.0];
        unsafe { sparse_set.get(entity.id() as usize).unwrap() }
    }

    /// # Safety
    /// Entity must be alive
    /// Component_id must correspond to a component array of type C
    pub(crate) unsafe fn get_mut_component_by_id<C: Component>(&mut self, entity: Entity, component_id: ComponentId) -> Option<&mut C> {
        let sparse_set = &mut self.components[component_id.0];
        unsafe { sparse_set.get_mut(entity.id() as usize) }
    }

    /// # Safety
    /// Entity must be alive
    /// Component_id must correspond to a component array of type C
    pub(crate) unsafe fn get_mut_component_by_id_unchecked<C: Component>(&mut self, entity: Entity, component_id: ComponentId) -> &mut C {
        let sparse_set = &mut self.components[component_id.0];
        unsafe { sparse_set.get_mut(entity.id() as usize).unwrap() }
    }

    #[inline]
    pub(crate) fn groups(&self) -> &HashMap<Signature, SparseSet<Entity>> {
        &self.groups
    }

    /// Entity must be alive
    pub(crate) fn get_entity_signature_by_type_id(&self, entity: Entity) -> Option<Signature> {
        self.entity_signatures.get(entity.id() as usize).copied()
    }
}
