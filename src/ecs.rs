use crate::{component_manager::ComponentManager, entity_manager::{EntityBundle, EntityManager}, system::Schedule, Component, Entity};
use std::cell::{Ref, RefMut};

#[derive(Default)]
pub struct ECS {
    pub component_manager: ComponentManager,
    pub entity_manager: EntityManager,
}

unsafe impl Sync for ECS {}
unsafe impl Send for ECS {}

impl ECS {
    pub fn spawn<T: EntityBundle>(&mut self, components: T) -> Option<Entity> {
        components.spawn(&mut self.entity_manager, &mut self.component_manager)
    }

    pub fn remove(&mut self, entity: Entity) {
        if self.entity_manager.remove(entity).is_ok() {
            self.component_manager.remove_entity(entity);
        }
    }

    #[inline]
    pub const fn is_alive(&self, entity: Entity) -> bool {
        self.entity_manager.is_alive(entity)
    }

    pub fn set_component<T: Component + 'static>(&mut self, entity: Entity, component: T) {
        self.component_manager.set_entity_component(entity, component);
    }

    pub fn get_component<T: Component + 'static>(&self, entity: Entity) -> Option<Ref<T>> {
        self.component_manager.get_entity_component(entity)
    }

    pub fn get_mut_component<T: Component + 'static>(&mut self, entity: Entity) -> Option<RefMut<T>> {
        self.component_manager.get_mut_entity_component(entity)
    }

    pub fn execute_schedule(&self, schedule: Schedule) {
        schedule.execute_all(&self.component_manager);
    }
}

pub enum ECSError {
    RemoveDeadEntity,
}
