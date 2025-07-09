use crate::{component_manager::ComponentManager, ecs::ECSError, Component, Entity, MAX_ENTITIES};

pub struct EntityManager {
    free_entities: Vec<Entity>,
    alive_entities: [bool; MAX_ENTITIES],
}

impl Default for EntityManager {
    fn default() -> Self {
        let free_entities = Vec::from_iter((0..MAX_ENTITIES).rev().map(|id| Entity::new(id as u16)));
        Self {
            free_entities,
            alive_entities: [false; MAX_ENTITIES],
        }
    }
}

impl EntityManager {
    pub fn spawn(&mut self) -> Option<Entity> {
        let entity = self.free_entities.pop()?;
        self.alive_entities[entity.id() as usize] = true;
        Some(entity)
    }

    pub fn remove(&mut self, entity: Entity) -> Result<(), ECSError> {
        if !self.is_alive(entity) { return Err(ECSError::RemoveDeadEntity); }
        self.alive_entities[entity.id() as usize] = false;
        self.free_entities.push(entity);
        Ok(())
    }

    #[inline]
    pub const fn is_alive(&self, entity: Entity) -> bool {
        self.alive_entities[entity.id() as usize]
    }
}

pub trait EntityBundle {
    type Data;
    fn spawn(self, entity_manager: &mut EntityManager, component_manager: &mut ComponentManager) -> Option<Entity>;
}

impl<T: Component + 'static> EntityBundle for T {
    type Data = T;
    fn spawn(self, entity_manager: &mut EntityManager, component_manager: &mut ComponentManager) -> Option<Entity> {
        let entity = entity_manager.spawn()?;
        component_manager.set_entity_component(entity, self);
        Some(entity)
    }
}

macro_rules! bundle_typle_impl {
    ($(($idx:tt, $name:ident)),+) => {
        impl<$($name: Component + 'static),+> EntityBundle for ($($name),+) {
            type Data = ($($name),+);
            fn spawn(self, entity_manager: &mut EntityManager, component_manager: &mut ComponentManager) -> Option<Entity> {
                let data = self;
                let entity = entity_manager.spawn()?;
                $(component_manager.set_entity_component(entity, data.$idx));+;
                Some(entity)
            }
        }
    }
}

variadics_please::all_tuples_enumerated!{bundle_typle_impl, 2, 32, C}
