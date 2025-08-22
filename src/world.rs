use std::any::TypeId;

use crate::*;

static WORLD_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct WorldId(usize);

pub struct World {
    id: WorldId,
    components: component::Components,
    entity_manager: entity::Entities,
    resources: resource::Resources,
    pub(crate) thread_pool: rayon::ThreadPool,
    system_command_receiver: std::sync::mpsc::Receiver<system::SystemCommand>,
    pub(crate) system_command_sender: std::sync::mpsc::Sender<system::SystemCommand>,
}

impl Default for World {
    fn default() -> Self {
        Self::new(Self::DEFAULT_THREAD_COUND).unwrap()
    }
}

unsafe impl Sync for World {}
unsafe impl Send for World {}

impl World {
    const DEFAULT_THREAD_COUND: usize = 16;
    pub fn new(num_threads: usize) -> Result<Self, rayon::ThreadPoolBuildError> {
        let (system_command_sender, system_command_receiver) = std::sync::mpsc::channel();
        let mut world = Self {
            id: WorldId(WORLD_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)),
            components: Default::default(),
            entity_manager: Default::default(),
            resources: Default::default(),
            thread_pool: rayon::ThreadPoolBuilder::new().num_threads(num_threads).build()?,
            system_command_receiver,
            system_command_sender,
        };

        world.insert_resource(schedule::Schedules::default());

        Ok(world)
    }

    pub fn id(&self) -> WorldId {
        self.id
    }

    
    // ===== Schedules =====

    pub fn run_schedule<L: schedule::ScheduleLabel>(&mut self, label: &L) {
        let mut schedules = unsafe { self.resources.get_mut_unsafe::<Schedules>().expect("Schedules not initialized") };
        let Some(schedule) = schedules.get_mut(label) else { return; };
        schedule.execute(self);
        self.handle_commands();
    }

    pub fn insert_schedule<L: schedule::ScheduleLabel>(&mut self, label: L, schedule: Schedule) {
        let mut schedules = self.get_mut_resource::<Schedules>().expect("Schedules not initialized");
        schedules.insert(label, schedule);
    }

    pub(crate) fn handle_commands(&mut self) {
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

    // ===== Resources =====

    pub fn insert_resource<R: Resource>(&mut self, resource: R) -> Option<R> {
        self.resources.insert(resource)
    }

    pub fn remove_resource<R: Resource>(&mut self) -> Option<R> {
        self.resources.remove()
    }

    pub fn get_resource<R: Resource>(&self) -> Option<Res<'_, R>> {
        self.resources.get::<R>()
    }

    pub fn get_mut_resource<R: Resource>(&mut self) -> Option<ResMut<'_, R>> {
        self.resources.get_mut::<R>()
    }

    /// # Safety
    /// caller must ensure that the borrow is safe
    pub unsafe fn get_mut_resource_unsafe<R: Resource>(&self) -> Option<ResMut<'_, R>> {
        unsafe { self.resources.get_mut_unsafe::<R>() }
    }

    pub fn resource<R: Resource>(&self) -> Res<'_, R> {
        self.resources.get()
            .unwrap_or_else(|| panic!("resource '{}' not initialized", std::any::type_name::<R>()))
    }

    pub fn resource_mut<R: Resource>(&mut self) -> ResMut<'_, R> {
        self.resources.get_mut()
            .unwrap_or_else(|| panic!("resource '{}' not initialized", std::any::type_name::<R>()))
    }

    /// # Safety
    /// caller must ensure that the borrow is safe
    pub unsafe fn resource_mut_unsafe<R: Resource>(&self) -> ResMut<'_, R> {
        unsafe { self.resources.get_mut_unsafe()
            .unwrap_or_else(|| panic!("resource '{}' not initialized", std::any::type_name::<R>())) }
    }

    pub fn get_resource_or_insert<R: Resource>(&mut self, default: R) -> ResMut<'_, R> {
        self.resources.get_or_insert(default)
    }

    // ===== Components =====

    pub fn register_component<C: Component>(&mut self) -> ComponentId {
        self.components.register_component::<C>()
    }

    pub fn set_component<C: Component>(&mut self, entity: Entity, component: C) {
        if !self.is_alive(entity) { return; }
        self.components.set_component(entity, component);
    }

    /// # Safety
    /// caller must ensure that the entity is alive and the given component exists
    pub unsafe fn set_component_unchecked<C: Component>(&mut self, entity: Entity, component: C) {
        unsafe { self.components.set_component_unchecked(entity, component) };
    }

    pub fn remove_component<C: Component>(&mut self, entity: Entity) {
        if !self.is_alive(entity) { return; }
        self.components.remove_component::<C>(entity);
    }

    pub fn get_component<C: Component>(&self, entity: Entity) -> Option<&C> {
        if !self.is_alive(entity) { return None; }
        self.components.get_component(entity)
    }

    pub fn get_mut_component<C: Component>(&mut self, entity: Entity) -> Option<&mut C> {
        if !self.is_alive(entity) { return None; }
        self.components.get_mut_component(entity)
    }

    /// # Safety
    /// caller must ensure that the borrow is safe
    pub unsafe fn get_component_by_id<C: Component>(&self, entity: Entity, component_id: ComponentId) -> Option<*mut C> {
        if !self.is_alive(entity) { return None; }
        unsafe { self.components.get_component_by_id(entity, component_id) }
    }

    /// # Safety
    /// caller must ensure that the borrow is safe and the entity is alive
    pub unsafe fn get_component_by_id_unchecked<C: Component>(&self, entity: Entity, component_id: ComponentId) -> *mut C {
        unsafe { self.components.get_component_by_id_unchecked(entity, component_id) }
    }

    pub fn groups(&self) -> &std::collections::HashMap<Signature, sparse_set::SparseSet<Entity>> {
        self.components.groups()
    }

    pub fn get_entity_signature(&self, entity: Entity) -> Option<Signature> {
        if !self.is_alive(entity) { return None; }
        self.components.get_entity_signature_by_type_id(entity)
    }

    pub fn get_component_signature_by_type_id(&self, type_id: &TypeId) -> Option<Signature> {
        self.components.get_component_signature(type_id)
    }

    pub fn get_component_id<C: Component>(&self) -> Option<ComponentId> {
        self.components.get_component_id::<C>()
    }

    // ===== Entities =====

    pub fn spawn<B: entity::EntityBundle>(&mut self, components: B) -> Entity {
        components.spawn(self)
    }
    
    /// # Safety
    /// caller must manually set the components
    pub unsafe fn spawn_with_signature(&mut self, signature: Signature) -> Entity {
        let entity = self.entity_manager.spawn();
        unsafe { self.components.insert_empty_entity(entity, signature) };
        entity
    }

    pub fn remove(&mut self, entity: Entity) {
        if self.entity_manager.is_alive(entity) {
            self.entity_manager.remove(entity);
            self.components.remove_entity(entity);
        }
    }

    #[inline]
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entity_manager.is_alive(entity)
    }
}
