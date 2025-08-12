use crate::bitmap::Bitmap;

use super::{Component, ECSError, EntityId, ECS, MAX_ENTITIES};

pub struct EntityManager {
    free_entities: Vec<EntityId>,
    alive_entities: [bool; MAX_ENTITIES],
}

impl Default for EntityManager {
    fn default() -> Self {
        let free_entities = Vec::from_iter((0..MAX_ENTITIES).rev().map(|id| EntityId::new(id as u32)));
        Self {
            free_entities,
            alive_entities: [false; MAX_ENTITIES],
        }
    }
}

impl EntityManager {
    pub fn spawn(&mut self) -> Option<EntityId> {
        let entity = self.free_entities.pop()?;
        self.alive_entities[entity.id() as usize] = true;
        Some(entity)
    }

    pub fn remove(&mut self, entity: EntityId) -> Result<(), ECSError> {
        if !self.is_alive(entity) { return Err(ECSError::RemoveDeadEntity); }
        self.alive_entities[entity.id() as usize] = false;
        self.free_entities.push(entity);
        Ok(())
    }

    #[inline]
    pub const fn is_alive(&self, entity: EntityId) -> bool {
        self.alive_entities[entity.id() as usize]
    }
}

pub trait EntityBundle {
    type Data;
    fn spawn(self, ecs: &mut ECS) -> Option<EntityId>;
}

impl<C: Component + 'static> EntityBundle for C {
    type Data = C;
    fn spawn(self, ecs: &mut ECS) -> Option<EntityId> {
        let entity_id = ecs.entity_manager.spawn()?;
        let signature = ecs.component_manager.register_component::<C>();
        ecs.component_manager.spawn_entity(entity_id, signature);
        unsafe { ecs.component_manager.set_component_limited_checks(entity_id, self) };
        Some(entity_id)
    }
}

macro_rules! bundle_typle_impl {
    ($(($idx:tt, $name:ident)),+) => {
        impl<$($name: Component + 'static),+> EntityBundle for ($($name),+) {
            type Data = ($($name),+);
            fn spawn(self, ecs: &mut ECS) -> Option<EntityId> {
                let data = self;
                let mut signature = Bitmap::new();
                $(signature |= ecs.component_manager.register_component::<$name>();)+
                let entity_id = ecs.entity_manager.spawn()?;
                ecs.component_manager.spawn_entity(entity_id, signature);
                unsafe {
                    $(ecs.component_manager.set_component_limited_checks(entity_id, data.$idx));+;
                }
                Some(entity_id)
            }
        }
    }
}

variadics_please::all_tuples_enumerated!{bundle_typle_impl, 2, 32, C}
