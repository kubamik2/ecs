use std::{any::TypeId, marker::PhantomData, ops::{Deref, DerefMut}, ptr::{self, NonNull}};

use crate::{observer::{ObserverInput, Observers, SignalInput}, query::QueryData, resource::{Changed, ResourceId}, schedule::Schedules, system::{IntoSystem, System, SystemId}, *};

static WORLD_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct WorldId(usize);

pub struct World {
    id: WorldId,
    components: component::Components,
    pub(crate) entities: entity::Entities,
    resources: resource::Resources,
    observers: Observers,
    schedules: Schedules,
    pub(crate) thread_pool: rayon::ThreadPool,
    command_buffer: Vec<u8>,
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
        Ok(Self {
            id: WorldId(WORLD_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)),
            components: Default::default(), entities: Default::default(),
            resources: Default::default(),
            observers: Default::default(),
            schedules: Schedules::default(),
            thread_pool: rayon::ThreadPoolBuilder::new().num_threads(num_threads).build()?,
            command_buffer: Vec::new(),
        })
    }

    #[inline]
    pub const fn id(&self) -> WorldId {
        self.id
    }

    #[inline]
    pub const fn world_ptr<'a>(&self) -> WorldPtr<'a> {
        WorldPtr {
            ptr: NonNull::new(ptr::from_ref(self).cast_mut()).expect("world pointer cast null"),
            allow_mutable_access: false,
            _m: PhantomData,
        }
    }

    #[inline]
    pub const fn world_ptr_mut<'a>(&mut self) -> WorldPtr<'a> {
        WorldPtr {
            ptr: NonNull::new(ptr::from_mut(self)).expect("world pointer cast null"),
            allow_mutable_access: true,
            _m: PhantomData,
        }
    }

    
    // ===== Schedules =====


    pub fn run_schedule<L: schedule::ScheduleLabel>(&mut self, label: L) {
        let mut world_ptr = self.world_ptr_mut();
        let world = unsafe { world_ptr.as_world_mut() };
        let Some(schedule) = world.schedules.get_mut(&label) else { return; };
        schedule.run(unsafe { world_ptr.as_world_mut() });
    }

    #[inline]
    pub fn insert_schedule<L: schedule::ScheduleLabel>(&mut self, label: L, schedule: Schedule) {
        self.schedules.insert(label, schedule);
    }

    #[inline]
    pub fn remove_schedule<L: schedule::ScheduleLabel>(&mut self, label: &L) -> Option<Schedule> {
        self.schedules.remove(label)
    }


    // ===== Resources =====
    

    #[inline]
    pub fn register_resource<R: Resource>(&mut self) -> ResourceId {
        self.resources.register::<R>()
    }

    #[inline]
    pub fn insert_resource<R: Resource>(&mut self, mut resource: R) -> Option<R> {
        resource.on_add(&mut self.command_buffer());
        match self.resources.insert(resource) {
            Some(mut res) => {
                res.on_remove(&mut self.command_buffer());
                self.process_command_buffer();
                Some(res)
            },
            None => {
                self.process_command_buffer();
                None
            }
        }
    }

    #[inline]
    pub fn remove_resource<R: Resource>(&mut self) -> Option<R> {
        if let Some(mut res) = self.resources.remove::<R>() {
            res.on_remove(&mut self.command_buffer());
            self.process_command_buffer();
            return Some(res);
        }
        None
    }

    #[inline]
    pub(crate) fn remove_resource_by_id(&mut self, resource_id: ResourceId) {
        let mut commands = Commands::new(&mut self.command_buffer, &self.entities);
        self.resources.remove_by_id(resource_id, &mut commands);
        self.process_command_buffer();
    }

    #[inline]
    pub fn get_resource<R: Resource>(&self) -> Option<&R> {
        self.resources.get::<R>()
    }

    #[inline]
    pub fn get_resource_mut<'a, R: Resource>(&mut self) -> Option<WorldResMut<'a, R>> {
        let mut world_ptr = self.world_ptr_mut();
        let val = unsafe { world_ptr.as_world_mut() }.resources.get_mut::<R>()?;
        Some(WorldResMut {
            val,
            world_ptr,
            was_modified: false
        })
    }

    #[inline]
    pub(crate) fn get_resource_ref_mut<R: Resource>(&mut self) -> Option<&'_ mut R> {
        self.resources.get_mut()
    }

    #[inline]
    pub fn resource<R: Resource>(&self) -> &R {
        self.resources.get()
            .unwrap_or_else(|| panic!("resource '{}' not initialized", std::any::type_name::<R>()))
    }

    #[inline]
    pub fn resource_mut<'a, R: Resource>(&mut self) -> WorldResMut<'a, R> {
        let mut world_ptr = self.world_ptr_mut();
        let val = unsafe { world_ptr.as_world_mut() }.resource_ref_mut::<R>();
        WorldResMut {
            val,
            world_ptr,
            was_modified: false
        }
    }

    #[inline]
    pub(crate) fn resource_ref_mut<R: Resource>(&mut self) -> &mut R {
        self.resources.get_mut()
            .unwrap_or_else(|| panic!("resource '{}' not initialized", std::any::type_name::<R>()))
    }

    #[inline]
    pub fn get_resource_or_insert_with<'a, R: Resource, F: FnOnce() -> R>(&mut self, f: F) -> WorldResMut<'a, R> {
        let mut world_ptr = self.world_ptr_mut();
        let mut commands = self.command_buffer();
        let val = unsafe { world_ptr.as_world_mut() }.resources.get_or_insert_with(|| {
            let mut resource = f();
            resource.on_add(&mut commands);
            resource
        });
        self.process_command_buffer();
        WorldResMut {
            val,
            world_ptr,
            was_modified: false
        }
    }

    #[inline]
    pub(crate) fn get_resource_ref_or_insert_with<R: Resource, F: FnOnce() -> R>(&mut self, f: F) -> &mut R {
        let mut world_ptr = self.world_ptr_mut();
        let mut commands = self.command_buffer();
        unsafe { world_ptr.as_world_mut() }.resources.get_or_insert_with(|| {
            let mut resource = f();
            resource.on_add(&mut commands);
            resource
        })
    }

    #[inline]
    pub fn get_resource_id<R: Resource>(&self) -> Option<ResourceId> {
        self.resources.get_resource_id::<R>()
    }

    #[inline]
    pub fn resource_id<R: Resource>(&self) -> ResourceId {
        self.resources.get_resource_id::<R>()
            .unwrap_or_else(|| panic!("resource '{}' not identified", std::any::type_name::<R>()))
    }

    #[inline]
    pub fn get_resource_by_id<R: Resource>(&self, id: ResourceId) -> Option<&R> {
        self.resources.get_resource_by_id(id)
    }

    #[inline]
    pub fn get_resource_by_id_mut<R: Resource>(&mut self, id: ResourceId) -> Option<&mut R> {
        self.resources.get_resource_by_id_mut(id)
    }

    #[inline]
    pub fn resource_by_id<R: Resource>(&self, id: ResourceId) -> &R {
        self.resources.get_resource_by_id(id)
            .unwrap_or_else(|| panic!("resource '{}' not initialized", std::any::type_name::<R>()))
    }

    #[inline]
    pub fn resource_by_id_mut<R: Resource>(&mut self, id: ResourceId) -> &mut R {
        self.resources.get_resource_by_id_mut(id)
            .unwrap_or_else(|| panic!("resource '{}' not initialized", std::any::type_name::<R>()))
    }


    // ===== Components =====


    #[inline]
    pub fn register_component<C: Component>(&mut self) -> ComponentId {
        self.components.register_component::<C>()
    }

    #[inline]
    pub fn set_component<C: Component>(&mut self, entity: Entity, mut component: C) {
        if !self.is_alive(entity) { return; }
        component.on_add(&mut self.command_buffer());
        if let Some(mut component) = self.components.set_component(entity, component) {
            component.on_remove(&mut self.command_buffer());
        }
        self.process_command_buffer();
    }

    /// # Safety
    /// Caller must ensure that the entity is alive and the given component exists
    #[inline]
    pub(crate) unsafe fn set_component_unchecked<C: Component>(&mut self, entity: Entity, mut component: C) {
        component.on_add(&mut self.command_buffer());
        if let Some(mut component) = unsafe { self.components.set_component_unchecked(entity, component) } {
            component.on_remove(&mut self.command_buffer());
        }
        self.process_command_buffer();
    }

    #[inline]
    pub fn remove_component<C: Component>(&mut self, entity: Entity) {
        if !self.is_alive(entity) { return; }
        if let Some(mut component) = self.components.remove_component::<C>(entity) {
            component.on_remove(&mut self.command_buffer());
        }
        self.process_command_buffer();
    }

    #[inline]
    pub fn get_component<C: Component>(&self, entity: Entity) -> Option<&C> {
        if !self.is_alive(entity) { return None; }
        self.components.get_component(entity)
    }

    #[inline]
    pub fn get_component_mut<C: Component>(&mut self, entity: Entity) -> Option<&mut C> {
        if !self.is_alive(entity) { return None; }
        self.components.get_mut_component(entity)
    }

    /// # Safety
    /// Entity must be alive
    /// Component_id must correspond to a component array of type C
    #[inline]
    pub unsafe fn get_component_by_id<C: Component>(&self, entity: Entity, component_id: ComponentId) -> Option<&C> {
        if !self.is_alive(entity) { return None; }
        unsafe { self.components.get_component_by_id(entity, component_id) }
    }

    /// # Safety
    /// Entity must be alive
    /// Component_id must correspond to a component array of type C
    #[inline]
    pub unsafe fn get_component_by_id_unchecked<C: Component>(&self, entity: Entity, component_id: ComponentId) -> &C {
        unsafe { self.components.get_component_by_id_unchecked(entity, component_id) }
    }

    /// # Safety
    /// Entity must be alive
    /// Component_id must correspond to a component array of type C
    #[inline]
    pub unsafe fn get_component_by_id_mut<C: Component>(&mut self, entity: Entity, component_id: ComponentId) -> Option<&mut C> {
        if !self.is_alive(entity) { return None; }
        unsafe { self.components.get_mut_component_by_id(entity, component_id) }
    }

    /// # Safety
    /// Entity must be alive
    /// Component_id must correspond to a component array of type C
    #[inline]
    pub unsafe fn get_component_by_id_unchecked_mut<C: Component>(&mut self, entity: Entity, component_id: ComponentId) -> &mut C {
        unsafe { self.components.get_mut_component_by_id_unchecked(entity, component_id) }
    }

    #[inline]
    pub fn groups(&self) -> &std::collections::HashMap<Signature, storage::sparse_set::SparseSet<Entity>> {
        self.components.groups()
    }

    #[inline]
    pub fn get_entity_signature(&self, entity: Entity) -> Option<Signature> {
        if !self.is_alive(entity) { return None; }
        self.components.get_entity_signature_by_type_id(entity)
    }

    #[inline]
    pub fn get_component_signature_by_type_id(&self, type_id: &TypeId) -> Option<Signature> {
        self.components.get_component_signature(type_id)
    }

    #[inline]
    pub fn get_component_id<C: Component>(&self) -> Option<ComponentId> {
        self.components.get_component_id::<C>()
    }

    #[inline]
    pub fn component_id<C: Component>(&self) -> ComponentId {
        self.components.get_component_id::<C>()
            .unwrap_or_else(|| panic!("component '{}' not identified", std::any::type_name::<C>()))
    }


    // ===== Entities =====


    #[inline]
    pub fn spawn<B: component::ComponentBundle>(&mut self, components: B) -> Entity {
        let entity = self.entities.spawn();
        components.spawn(entity, self);
        entity
    }

    #[inline]
    pub(crate) fn spawn_reserved<B: component::ComponentBundle>(&mut self, entity: Entity, components: B) {
        components.spawn(entity, self)
    }

    #[inline]
    pub(crate) unsafe fn insert_empty_entity(&mut self, entity: Entity, signature: Signature) {
        unsafe { self.components.insert_empty_entity(entity, signature);}
    }

    pub fn despawn(&mut self, entity: Entity) {
        if self.entities.is_alive(entity) {
            self.entities.despawn(entity, &mut self.command_buffer);
            self.components.despawn(entity, Commands::new(&mut self.command_buffer, &self.entities));
        }
        self.process_command_buffer();
    }

    #[inline]
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entities.is_alive(entity)
    }

    pub fn add_child(&mut self, parent: Entity, child: Entity) {
        if !self.is_alive(parent) || !self.is_alive(child) { return; }
        self.entities.add_child(parent, child);
    }

    pub fn remove_child(&mut self, parent: Entity, child: Entity) {
        if !self.is_alive(parent) { return; }
        self.entities.remove_child(parent, child);
    }

    pub fn remove_children(&mut self, entity: Entity) {
        if !self.is_alive(entity) { return; }
        self.entities.remove_children(entity);
    }

    pub fn children(&self, entity: Entity) -> &[Entity] {
        if !self.is_alive(entity) { return &[]; }
        self.entities.children(entity)
    }

    // ===== Signals =====
    

    pub(crate) fn send_signal_from_system<E: Event>(&mut self, event: E, target: Option<Entity>) {
        let mut world_ptr = self.world_ptr_mut();
        unsafe { world_ptr.as_world_mut() }.observers.send_signal(event, target, world_ptr);
    }

    pub fn send_signal<E: Event>(&mut self, event: E, target: Option<Entity>) {
        self.remove_dead_observers();
        let mut world_ptr = self.world_ptr_mut();
        unsafe { world_ptr.as_world_mut() }.observers.send_signal(event, target, world_ptr);
    }


    // ===== Observers =====


    #[inline]
    pub fn add_observer<ParamIn: ObserverInput, S: IntoSystem<ParamIn, SignalInput> + 'static>(&mut self, system: S) -> SystemId {
        self.observers.add_observer(system)
    }

    #[inline]
    pub fn add_observer_with_id<ParamIn: ObserverInput, S: IntoSystem<ParamIn, SignalInput> + 'static>(&mut self, system: S, id: SystemId) {
        let boxed_system = Box::new(system.into_system_with_id(id));
        self.observers.add_boxed_observer(boxed_system);
    }

    #[inline]
    pub(crate) fn remove_dead_observers(&mut self) {
        self.observers.remove_dead_observers();
    }


    // ===== Systems =====


    #[inline]
    pub fn add_system<L: ScheduleLabel, ParamIn: SystemInput, S: IntoSystem<ParamIn, ()> + 'static>(&mut self, label: L, system: S) -> SystemId {
        let schedule = self.schedules.get_or_default(label);
        let boxed_system = Box::new(system.into_system());
        let id = boxed_system.id().clone();
        schedule.add_boxed_system(boxed_system);
        id
    }

    #[inline]
    pub(crate) fn add_system_with_id<L: ScheduleLabel, ParamIn: SystemInput, S: IntoSystem<ParamIn, ()> + 'static>(&mut self, label: L, system: S, id: SystemId) {
        let schedule = self.schedules.get_or_default(label);
        let boxed_system = Box::new(system.into_system_with_id(id));
        schedule.add_boxed_system(boxed_system);
    }


    // ===== Events =====


    #[inline]
    pub fn send_event<E: Event>(&mut self, event: E) {
        let event_queue = self.get_resource_ref_or_insert_with(|| EventQueue::<E>::new());
        event_queue.send(event);
    }

    #[inline]
    pub fn register_event<E: Event>(&mut self) {
        self.get_resource_or_insert_with(|| EventQueue::<E>::new());
    }

    
    // ===== Other =====
    

    #[inline]
    pub fn query<D: QueryData>(&mut self) -> Query<'_, D, ()> {
        Query::new(self)
    }

    #[inline]
    pub fn query_filtered<D: QueryData, F: QueryFilter>(&mut self) -> Query<'_, D, F> {
        Query::new(self)
    }

    #[inline]
    pub(crate) const fn command_buffer(&mut self) -> Commands<'_> {
        Commands::new(&mut self.command_buffer, &self.entities)
    }

    #[inline]
    pub(crate) fn process_command_buffer(&mut self) {
        let mut queue = Vec::new();
        std::mem::swap(&mut queue, &mut self.command_buffer);
        Commands::process(&mut queue, self);
    }
}

#[derive(Clone, Copy)]
pub struct WorldPtr<'a> {
    ptr: NonNull<World>,
    allow_mutable_access: bool,
    _m: PhantomData<&'a mut World>,
}

unsafe impl Sync for WorldPtr<'_> {}
unsafe impl Send for WorldPtr<'_> {}

impl<'a> WorldPtr<'a> {
    #[inline]
    pub const unsafe fn as_world(&self) -> &'a World {
        unsafe { self.ptr.as_ref() }
    }

    #[inline]
    pub const unsafe fn as_world_mut(&mut self) -> &'a mut World {
        debug_assert!(self.allow_mutable_access);
        unsafe { self.ptr.as_mut() }
    }

    #[inline]
    pub const fn demote(&mut self) {
        self.allow_mutable_access = false;
    }
}

pub struct WorldResMut<'a, R: Resource> {
    world_ptr: WorldPtr<'a>,
    val: &'a mut R,
    was_modified: bool,
}

impl<R: Resource> Drop for WorldResMut<'_, R> {
    fn drop(&mut self) {
        let world = unsafe { self.world_ptr.as_world_mut() };
        if self.was_modified {
            world.send_event(Changed::<R>::new());
            world.send_signal(Changed::<R>::new(), None);
        }
    }
}

impl<R: Resource> Deref for WorldResMut<'_, R> {
    type Target = R;
    fn deref(&self) -> &Self::Target {
        self.val
    }
}

impl<R: Resource> DerefMut for WorldResMut<'_, R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.was_modified = true;
        self.val
    }
}
