#![feature(sync_unsafe_cell, downcast_unchecked)]
mod bitmap;
mod component_manager; mod entity_manager;
mod system;
mod query;
mod param;
mod access;
mod resource;
mod sparse_set;
mod schedule;
mod signal;
mod event;
mod observer_manager;

pub use query::Query;
pub use resource::{Res, ResMut};
pub use derive::{Component, Resource};
pub use schedule::{Schedule, Schedules};
pub use system::Commands;
pub use signal::Signal;

pub const MAX_ENTITIES: usize = 2_usize.pow(16)-1;
pub const MAX_COMPONENTS: usize = 128;

#[derive(Hash, Clone, Copy, PartialEq, Eq)]
pub struct Entity {
    id: u16,
    version: u16,
}

impl Entity {
    pub(crate) const fn new(id: u16, version: u16) -> Self {
        assert!(id as usize <= MAX_ENTITIES);
        Self { id, version }
    }

    #[inline(always)]
    pub const fn version(&self) -> u16 {
        self.version
    }

    #[inline(always)]
    pub const fn id(&self) -> u16 {
        self.id
    }

    pub(crate) const fn increment_version(&mut self) {
        self.version += 1;
    }
}

impl std::fmt::Debug for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}v{}", self.id(), self.version()))
    }
}

pub trait Component: Send + Sync + 'static {}
pub trait Resource: Send + Sync + 'static {}

pub struct ECS {
    pub(crate) component_manager: component_manager::ComponentManager,
    pub(crate) entity_manager: entity_manager::EntityManager,
    pub(crate) resource_manager: resource::ResourceManager,
    pub(crate) thread_pool: rayon::ThreadPool,
    system_command_receiver: std::sync::mpsc::Receiver<system::SystemCommand>,
    pub(crate) system_command_sender: std::sync::mpsc::Sender<system::SystemCommand>,
}

impl Default for ECS {
    fn default() -> Self {
        Self::new(Self::DEFAULT_THREAD_COUND).unwrap()
    }
}

unsafe impl Sync for ECS {}
unsafe impl Send for ECS {}

impl ECS {
    const DEFAULT_THREAD_COUND: usize = 16;
    pub fn new(num_threads: usize) -> Result<Self, rayon::ThreadPoolBuildError> {
        let (system_command_sender, system_command_receiver) = std::sync::mpsc::channel();
        let mut ecs = Self {
            component_manager: Default::default(),
            entity_manager: Default::default(),
            resource_manager: Default::default(),
            thread_pool: rayon::ThreadPoolBuilder::new().num_threads(num_threads).build()?,
            system_command_receiver,
            system_command_sender,
        };

        ecs.insert_resource(schedule::Schedules::default());

        Ok(ecs)
    }

    pub fn spawn<B: entity_manager::EntityBundle>(&mut self, components: B) -> Entity {
        components.spawn(self)
    }

    pub fn remove(&mut self, entity: Entity) {
        if self.entity_manager.is_alive(entity) {
            self.entity_manager.remove(entity);
            self.component_manager.remove_entity(entity);
        }
    }

    #[inline]
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entity_manager.is_alive(entity)
    }

    pub fn set_component<C: Component>(&mut self, entity: Entity, component: C) {
        self.component_manager.set_component(entity, component);
    }

    pub fn remove_component<C: Component>(&mut self, entity: Entity) {
        self.component_manager.remove_component::<C>(entity);
    }

    pub fn get_component<C: Component>(&self, entity: Entity) -> Option<&C> {
        self.component_manager.get_component(entity)
    }

    pub fn get_mut_component<T: Component>(&mut self, entity: Entity) -> Option<&mut T> {
        self.component_manager.get_mut_component(entity)
    }

    pub fn run_schedule<L: schedule::ScheduleLabel>(&mut self, label: &L) {
        let schedules = self.get_resource::<Schedules>().expect("Schedules not initialized");
        let Some(schedule) = schedules.get(label) else { return; };
        schedule.execute(self);
        self.handle_commands();
    }

    pub fn insert_resource<R: Resource>(&mut self, resource: R) {
        self.resource_manager.insert(resource);
    }

    pub fn remove_resource<R: Resource>(&mut self) -> Option<R> {
        self.resource_manager.remove()
    }

    pub fn get_resource<R: Resource>(&self) -> Option<Res<'_, R>> {
        self.resource_manager.get::<R>()
    }

    pub fn get_mut_resource<R: Resource>(&mut self) -> Option<ResMut<'_, R>> {
        self.resource_manager.get_mut::<R>()
    }

    pub fn insert_schedule<L: schedule::ScheduleLabel>(&mut self, label: L, schedule: Schedule) {
        let mut schedules = self.get_mut_resource::<Schedules>().expect("Schedules not initialized");
        schedules.insert(label, schedule);
    }

    fn handle_commands(&mut self) {
        while let Ok(command) = self.system_command_receiver.try_recv() {
            match command {
                crate::system::SystemCommand::Spawn(spawn) => {
                    (spawn)(self)
                },
                crate::system::SystemCommand::Remove(entity) => {
                    self.remove(entity);
                },
                crate::system::SystemCommand::SetComponent(set_component) => {
                    (set_component)(self);
                },
                crate::system::SystemCommand::RemoveComponent(remove_component) => {
                    (remove_component)(self);
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component)]
    struct ComponentA {
        a: u128,
        b: u64,
        c: u32,
        d: u16,
        e: u8,
    }

    impl ComponentA {
        fn new(i: usize) -> Self {
            Self {
                a: i as u128,
                b: i as u64,
                c: i as u32,
                d: i as u16,
                e: i as u8,
            }
        }
    }

    impl ComponentA {
        fn validate(&self, i: usize) -> bool {
            let a = self.a;
            let b = self.b as u128;
            let c = self.c as u128;
            let d = self.d as u128;
            let e = self.e as u128;
            i as u128 == a && a == b && b == c && c == d && d == e
        }
    }

    #[derive(Component)]
    struct ComponentB(String);
    impl ComponentB {
        fn validate(&self, i: usize) -> bool {
            format!("{i}") == self.0
        }
    }

    #[test]
    fn get_component() {
        let mut ecs = ECS::default();
        let mut entities = vec![];
        for i in 0..100 {
            let entity = ecs.spawn((
                ComponentA::new(i),
                ComponentB(format!("{i}")),
            ));
            entities.push(entity);
        }
        (0..100).for_each(|i| {
            let component = ecs.get_component::<ComponentA>(entities[i]).expect("get_component ComponentA not found");
            assert!(component.validate(i), "ComponentA validation failed");
        });
        (0..100).for_each(|i| {
            let component = ecs.get_component::<ComponentB>(entities[i]).expect("get_component ComponentB not found");
            assert!(component.validate(i), "ComponentB validation failed");
        });
    }

    #[test]
    fn set_component() {
        let mut ecs = ECS::default();
        let mut entities = vec![];
        for i in 0..100 {
            let entity = ecs.spawn((
                ComponentA::new(i),
                ComponentB(format!("{i}")),
            ));
            entities.push(entity);
        }
        (0..100).for_each(|i| {
            ecs.set_component(entities[i], ComponentA::new(i+1));
            ecs.set_component(entities[i], ComponentB(format!("{}", i+1)));
        });
        (0..100).for_each(|i| {
            let component = ecs.get_component::<ComponentA>(entities[i]).unwrap();
            assert!(component.validate(i+1), "ComponentA validation failed");
        });
        (0..100).for_each(|i| {
            let component = ecs.get_component::<ComponentB>(entities[i]).unwrap();
            assert!(component.validate(i+1), "ComponentB validation failed");
        });
    }
}
