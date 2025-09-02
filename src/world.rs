use std::{any::TypeId, marker::PhantomData, ptr::{self, NonNull}};

use crate::{observer::{ObserverInput, Observers, SignalInput}, resource::ResourceId, system::{IntoSystem, System, SystemId, SystemValidationError}, *};

static WORLD_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct WorldId(usize);

pub struct World {
    id: WorldId,
    components: component::Components,
    entity_manager: entity::Entities,
    resources: resource::Resources,
    observers: Observers,
    pub(crate) thread_pool: rayon::ThreadPool,
    pub(crate) marked_sytems_for_removal: Vec<SystemId>,
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
        let mut world = Self {
            id: WorldId(WORLD_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)),
            components: Default::default(),
            entity_manager: Default::default(),
            resources: Default::default(),
            observers: Default::default(),
            thread_pool: rayon::ThreadPoolBuilder::new().num_threads(num_threads).build()?,
            marked_sytems_for_removal: Default::default(),
        };

        world.insert_resource(schedule::Schedules::default());

        Ok(world)
    }

    pub fn id(&self) -> WorldId {
        self.id
    }

    pub fn world_ptr(&self) -> WorldPtr<'_> {
        WorldPtr {
            ptr: NonNull::new(ptr::from_ref(self).cast_mut()).expect("world pointer cast null"),
            allow_mutable_access: false,
            _m: PhantomData,
        }
    }

    pub fn world_ptr_mut(&mut self) -> WorldPtr<'_> {
        WorldPtr {
            ptr: NonNull::new(ptr::from_mut(self)).expect("world pointer cast null"),
            allow_mutable_access: true,
            _m: PhantomData,
        }
    }

    
    // ===== Schedules =====


    pub fn run_schedule<L: schedule::ScheduleLabel>(&mut self, label: &L) {
        let mut world_ptr = self.world_ptr_mut();
        let mut schedules = unsafe { world_ptr.as_world_mut() }.resource_mut::<Schedules>();
        let Some(schedule) = schedules.get_mut(label) else { return; };
        schedule.run(unsafe { world_ptr.as_world_mut() });
    }

    pub fn insert_schedule<L: schedule::ScheduleLabel>(&mut self, label: L, schedule: Schedule) {
        let mut schedules = self.resource_mut::<Schedules>();
        schedules.insert(label, schedule);
    }

    pub fn queue_system_removal(&mut self, system_id: SystemId) {
        self.marked_sytems_for_removal.push(system_id);
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

    pub fn resource<R: Resource>(&self) -> Res<'_, R> {
        self.resources.get()
            .unwrap_or_else(|| panic!("resource '{}' not initialized", std::any::type_name::<R>()))
    }

    pub fn resource_mut<R: Resource>(&mut self) -> ResMut<'_, R> {
        self.resources.get_mut()
            .unwrap_or_else(|| panic!("resource '{}' not initialized", std::any::type_name::<R>()))
    }

    pub fn get_resource_or_insert<R: Resource>(&mut self, default: R) -> ResMut<'_, R> {
        self.resources.get_or_insert(default)
    }

    pub fn get_resource_or_insert_with<R: Resource, F: FnOnce() -> R>(&mut self, f: F) -> ResMut<'_, R> {
        self.resources.get_or_insert_with(f)
    }

    pub fn get_resource_id<R: Resource>(&self) -> Option<ResourceId> {
        self.resources.get_resource_id::<R>()
    }

    pub fn resource_id<R: Resource>(&self) -> ResourceId {
        self.resources.get_resource_id::<R>()
            .unwrap_or_else(|| panic!("resource '{}' not identified", std::any::type_name::<R>()))
    }

    /// # Safety
    /// caller must ensure that the borrow is safe
    pub unsafe fn get_resource_by_id<R: Resource>(&self, id: ResourceId) -> Option<Res<R>> {
        unsafe { self.resources.get_resource_by_id(id) }
    }

    /// # Safety
    /// caller must ensure that the borrow is safe
    pub unsafe fn get_mut_resource_by_id<R: Resource>(&mut self, id: ResourceId) -> Option<ResMut<R>> {
        unsafe { self.resources.get_mut_resource_by_id(id) }
    }

    /// # Safety
    /// caller must ensure that the borrow is safe
    pub unsafe fn resource_by_id<R: Resource>(&self, id: ResourceId) -> Res<R> {
        unsafe { self.resources.get_resource_by_id(id)
            .unwrap_or_else(|| panic!("resource '{}' not initialized", std::any::type_name::<R>())) }
    }

    /// # Safety
    /// caller must ensure that the borrow is safe
    pub unsafe fn resource_mut_by_id<R: Resource>(&mut self, id: ResourceId) -> ResMut<R> {
        unsafe { self.resources.get_mut_resource_by_id(id)
            .unwrap_or_else(|| panic!("resource '{}' not initialized", std::any::type_name::<R>())) }
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
    pub unsafe fn get_component_by_id<C: Component>(&self, entity: Entity, component_id: ComponentId) -> Option<&C> {
        if !self.is_alive(entity) { return None; }
        unsafe { self.components.get_component_by_id(entity, component_id) }
    }

    /// # Safety
    /// caller must ensure that the borrow is safe and the entity is alive
    pub unsafe fn get_component_by_id_unchecked<C: Component>(&self, entity: Entity, component_id: ComponentId) -> &C {
        unsafe { self.components.get_component_by_id_unchecked(entity, component_id) }
    }

    /// # Safety
    /// caller must ensure that the borrow is safe
    pub unsafe fn get_mut_component_by_id<C: Component>(&mut self, entity: Entity, component_id: ComponentId) -> Option<&mut C> {
        if !self.is_alive(entity) { return None; }
        unsafe { self.components.get_mut_component_by_id(entity, component_id) }
    }

    /// # Safety
    /// caller must ensure that the borrow is safe and the entity is alive
    pub unsafe fn get_mut_component_by_id_unchecked<C: Component>(&mut self, entity: Entity, component_id: ComponentId) -> &mut C {
        unsafe { self.components.get_mut_component_by_id_unchecked(entity, component_id) }
    }

    pub fn groups(&self) -> &std::collections::HashMap<Signature, storage::sparse_set::SparseSet<Entity>> { self.components.groups()
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

    pub fn component_id<C: Component>(&self) -> ComponentId {
        self.components.get_component_id::<C>()
            .unwrap_or_else(|| panic!("component '{}' not identified", std::any::type_name::<C>()))
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


    pub fn add_observer<ParamIn: ObserverInput, S: IntoSystem<ParamIn, SignalInput> + 'static>(&mut self, system: S) -> Result<(), SystemValidationError> {
        let mut system: Box<dyn System<Input = SignalInput> + Send + Sync> = Box::new(system.into_system());
        system.init(self);
        self.observers.add_boxed_observer(system);
        Ok(())
    }

    #[inline]
    pub(crate) fn remove_dead_observers(&mut self) {
        self.observers.remove_dead_observers();
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
        assert!(self.allow_mutable_access);
        unsafe { self.ptr.as_mut() }
    }

    #[inline]
    pub const fn demote(&mut self) {
        self.allow_mutable_access = false;
    }
}
