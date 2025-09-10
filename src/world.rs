use std::{any::TypeId, marker::PhantomData, ptr::{self, NonNull}};

use crate::{observer::{ObserverInput, Observers, SignalInput}, query::QueryData, resource::ResourceId, schedule::Schedules, system::{IntoSystem, System, SystemId, SYSTEM_IDS}, *};

static WORLD_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct WorldId(usize);

pub struct World {
    id: WorldId,
    components: component::Components,
    entity_manager: entity::Entities,
    resources: resource::Resources,
    observers: Observers,
    schedules: Schedules,
    pub(crate) thread_pool: rayon::ThreadPool,
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
            components: Default::default(),
            entity_manager: Default::default(),
            resources: Default::default(),
            observers: Default::default(),
            schedules: Schedules::default(),
            thread_pool: rayon::ThreadPoolBuilder::new().use_current_thread().num_threads(num_threads).build()?,
        })
    }

    #[inline]
    pub const fn id(&self) -> WorldId {
        self.id
    }

    #[inline]
    pub const fn world_ptr(&self) -> WorldPtr<'_> {
        WorldPtr {
            ptr: NonNull::new(ptr::from_ref(self).cast_mut()).expect("world pointer cast null"),
            allow_mutable_access: false,
            _m: PhantomData,
        }
    }

    #[inline]
    pub const fn world_ptr_mut(&mut self) -> WorldPtr<'_> {
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
    pub fn insert_resource<R: Resource>(&mut self, resource: R) -> Option<R> {
        self.resources.insert(resource)
    }

    #[inline]
    pub fn remove_resource<R: Resource>(&mut self) -> Option<R> {
        self.resources.remove()
    }

    #[inline]
    pub fn get_resource<R: Resource>(&self) -> Option<Res<'_, R>> {
        self.resources.get::<R>()
    }

    #[inline]
    pub fn get_resource_mut<R: Resource>(&mut self) -> Option<ResMut<'_, R>> {
        self.resources.get_mut::<R>()
    }

    #[inline]
    pub fn resource<R: Resource>(&self) -> Res<'_, R> {
        self.resources.get()
            .unwrap_or_else(|| panic!("resource '{}' not initialized", std::any::type_name::<R>()))
    }

    #[inline]
    pub fn resource_mut<R: Resource>(&mut self) -> ResMut<'_, R> {
        self.resources.get_mut()
            .unwrap_or_else(|| panic!("resource '{}' not initialized", std::any::type_name::<R>()))
    }

    #[inline]
    pub fn get_resource_or_insert<R: Resource>(&mut self, default: R) -> ResMut<'_, R> {
        self.resources.get_or_insert(default)
    }

    #[inline]
    pub fn get_resource_or_insert_with<R: Resource, F: FnOnce() -> R>(&mut self, f: F) -> ResMut<'_, R> {
        self.resources.get_or_insert_with(f)
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
    pub fn get_resource_by_id<R: Resource>(&self, id: ResourceId) -> Option<Res<R>> {
        self.resources.get_resource_by_id(id)
    }

    #[inline]
    pub fn get_resource_by_id_mut<R: Resource>(&mut self, id: ResourceId) -> Option<ResMut<R>> {
        self.resources.get_mut_resource_by_id(id)
    }

    #[inline]
    pub fn resource_by_id<R: Resource>(&self, id: ResourceId) -> Res<R> {
        self.resources.get_resource_by_id(id)
            .unwrap_or_else(|| panic!("resource '{}' not initialized", std::any::type_name::<R>()))
    }

    #[inline]
    pub fn resource_by_id_mut<R: Resource>(&mut self, id: ResourceId) -> ResMut<R> {
        self.resources.get_mut_resource_by_id(id)
            .unwrap_or_else(|| panic!("resource '{}' not initialized", std::any::type_name::<R>()))
    }


    // ===== Components =====


    #[inline]
    pub fn register_component<C: Component>(&mut self) -> ComponentId {
        self.components.register_component::<C>()
    }

    #[inline]
    pub fn set_component<C: Component>(&mut self, entity: Entity, component: C) {
        if !self.is_alive(entity) { return; }
        self.components.set_component(entity, component);
    }

    /// # Safety
    /// Caller must ensure that the entity is alive and the given component exists
    #[inline]
    pub(crate) unsafe fn set_component_unchecked<C: Component>(&mut self, entity: Entity, component: C) {
        unsafe { self.components.set_component_unchecked(entity, component) };
    }

    #[inline]
    pub fn remove_component<C: Component>(&mut self, entity: Entity) {
        if !self.is_alive(entity) { return; }
        self.components.remove_component::<C>(entity);
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
    pub fn spawn<B: entity::EntityBundle>(&mut self, components: B) -> Entity {
        components.spawn(self)
    }
    
    /// # Safety
    /// caller must manually set the components
    #[inline]
    pub(crate) unsafe fn spawn_with_signature(&mut self, signature: Signature) -> Entity {
        let entity = self.entity_manager.spawn();
        unsafe { self.components.insert_empty_entity(entity, signature) };
        entity
    }

    pub fn despawn(&mut self, entity: Entity) {
        if self.entity_manager.is_alive(entity) {
            self.entity_manager.despawn(entity);
            self.components.despawn(entity);
        }
    }

    #[inline]
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entity_manager.is_alive(entity)
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


    pub fn add_observer<ParamIn: ObserverInput, S: IntoSystem<ParamIn, SignalInput> + 'static>(&mut self, system: S) -> SystemId {
        let mut system: Box<dyn System<Input = SignalInput> + Send + Sync> = Box::new(system.into_system());
        let id = system.id();
        system.init(self);
        self.observers.add_boxed_observer(system);
        id
    }

    #[inline]
    pub(crate) fn remove_dead_observers(&mut self) {
        self.observers.remove_dead_observers();
    }

    // ===== Systems =====


    #[inline]
    pub fn remove_system(&self, system_id: SystemId) {
        SYSTEM_IDS.write().unwrap().despawn(system_id.get());
    }

    #[inline]
    pub fn add_system<L: ScheduleLabel, ParamIn: SystemInput, S: IntoSystem<ParamIn, ()> + 'static>(&mut self, label: L, system: S) {
        let schedule = self.schedules.get_or_default(label);
        schedule.add_system(system);
    }


    // ===== Events =====


    #[inline]
    pub fn send_event<E: Event>(&mut self, event: E) {
        let mut event_queue = self.get_resource_or_insert_with(|| EventQueue::<E>::new());
        event_queue.send(event);
    }

    #[inline]
    pub fn register_event<E: Event>(&mut self) {
        self.get_resource_or_insert_with(|| EventQueue::<E>::new());
    }

    
    // ===== Other =====
    

    #[inline]
    pub fn query<D: QueryData>(&mut self) -> Query<'_, D> {
        Query::new(self)
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
    pub unsafe fn as_world(&self) -> &'a World {
        unsafe { self.ptr.as_ref() }
    }

    #[inline]
    pub unsafe fn as_world_mut(&mut self) -> &'a mut World {
        debug_assert!(self.allow_mutable_access);
        unsafe { self.ptr.as_mut() }
    }

    #[inline]
    pub const fn demote(&mut self) {
        self.allow_mutable_access = false;
    }
}
