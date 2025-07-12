use crate::{component_manager::ComponentManager, entity_manager::{EntityBundle, EntityManager}, resource::ResourceManager, system::Schedule, Component, Entity, Resource};

pub struct ECS {
    component_manager: ComponentManager,
    entity_manager: EntityManager,
    resource_manager: ResourceManager,
    thread_pool: rayon::ThreadPool,
}

impl Default for ECS {
    fn default() -> Self {
        ECS {
            component_manager: Default::default(),
            entity_manager: Default::default(),
            resource_manager: Default::default(),
            thread_pool: rayon::ThreadPoolBuilder::new().num_threads(16).build().unwrap(),
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
        })
    }

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

    pub fn get_component<T: Component + 'static>(&self, entity: Entity) -> Option<&T> {
        unsafe { self.component_manager.get_entity_component::<T>(entity).map(|f| f.as_ref().unwrap()) }
    }

    pub fn get_mut_component<T: Component + 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        unsafe { self.component_manager.get_mut_entity_component::<T>(entity).map(|f| f.as_mut().unwrap()) }
    }

    pub fn execute_schedule(&self, schedule: &Schedule) {
        schedule.execute(&self.component_manager, &self.resource_manager, &self.thread_pool);
    }

    pub fn insert_resource<R: Resource + Send + Sync + 'static>(&mut self, resource: R) {
        self.resource_manager.set(resource);
    }

    pub fn remove_resource<R: Resource + Send + Sync + 'static>(&mut self) -> Option<Box<R>> {
        self.resource_manager.remove()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ECSError {
    RemoveDeadEntity,
    MultipleMutRefs,
    IncompatibleRefs,
}
