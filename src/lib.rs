#![feature(sync_unsafe_cell, downcast_unchecked)]
mod bitmap;
mod component_manager; mod entity_manager;
mod system;
mod query;
mod param;
mod access;
mod resource;
mod sparse_set;

use std::collections::HashMap;

pub use query::Query;
pub use system::Schedule;
pub use resource::{Res, ResMut};
pub use derive::{Component, Resource};

pub const MAX_ENTITIES: usize = 8192 * 128;
pub const MAX_COMPONENTS: usize = 128;

#[derive(Hash, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityId(u32);

impl EntityId {
    pub(crate) const fn new(id: u32) -> Self {
        Self(id)
    }

    pub const fn id(&self) -> u32 {
        self.0
    }
}

pub trait Component: Send + Sync + 'static {}
pub trait Resource: Send + Sync + 'static {}

pub struct ECS {
    pub(crate) component_manager: component_manager::ComponentManager,
    pub(crate) entity_manager: entity_manager::EntityManager,
    pub(crate) resource_manager: resource::ResourceManager,
    pub(crate) thread_pool: rayon::ThreadPool,
    schedules: HashMap<String, Schedule>,
}

impl Default for ECS {
    fn default() -> Self {
        ECS {
            component_manager: Default::default(),
            entity_manager: Default::default(),
            resource_manager: Default::default(),
            thread_pool: rayon::ThreadPoolBuilder::new().num_threads(16).build().unwrap(),
            schedules: Default::default(),
        }
    }
}

unsafe impl Sync for ECS {}
unsafe impl Send for ECS {}

impl ECS {
    pub fn new(num_threads: usize) -> Result<Self, rayon::ThreadPoolBuildError> {
        Ok(Self {
            component_manager: Default::default(),
            entity_manager: Default::default(),
            resource_manager: Default::default(),
            thread_pool: rayon::ThreadPoolBuilder::new().num_threads(num_threads).build()?,
            schedules: Default::default(),
        })
    }

    pub fn spawn<T: entity_manager::EntityBundle>(&mut self, components: T) -> Option<EntityId> {
        components.spawn(self)
    }

    pub fn remove(&mut self, entity_id: EntityId) {
        if self.entity_manager.remove(entity_id).is_ok() {
            self.component_manager.remove_entity(entity_id);
        }
    }

    #[inline]
    pub const fn is_alive(&self, entity_id: EntityId) -> bool {
        self.entity_manager.is_alive(entity_id)
    }

    pub fn set_component<C: Component>(&mut self, entity_id: EntityId, component: C) {
        self.component_manager.set_component(entity_id, component);
    }

    pub fn get_component<C: Component>(&self, entity_id: EntityId) -> Option<&C> {
        self.component_manager.get_component(entity_id)
    }

    pub fn get_mut_component<T: Component>(&mut self, entity_id: EntityId) -> Option<&mut T> {
        self.component_manager.get_mut_component(entity_id)
    }

    pub fn execute_schedule(&self, name: &str) {
        if let Some(schedule) = self.schedules.get(name) {
            schedule.execute(self);
        }
    }

    pub fn insert_resource<R: Resource>(&mut self, resource: R) {
        self.resource_manager.insert(resource);
    }

    pub fn remove_resource<R: Resource>(&mut self) -> Option<Box<R>> {
        self.resource_manager.remove()
    }

    pub fn insert_schedule(&mut self, name: String, schedule: Schedule) {
        self.schedules.insert(name, schedule);
    }

    pub fn get_mut_schedule(&mut self, name: &str) -> Option<&mut Schedule> {
        self.schedules.get_mut(name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ECSError {
    RemoveDeadEntity,
    MultipleMutRefs,
    IncompatibleRefs,
}
