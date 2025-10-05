use crate::{Component, ComponentBundle, Entity, Event, IntoSystem, ObserverInput, Resource, ResourceId, ScheduleLabel, SignalInput, SystemInput, World, entity::Entities, param::SystemParam, world::WorldPtr};

use super::{SystemHandle, SystemId};

pub struct Commands<'a> {
    queue: &'a mut Vec<u8>,
    entities: &'a Entities,
}

enum CommandMeta {
    Spawn {
        f: fn(&mut World, *mut u8, Entity),
        data_size: usize,
        entity: Entity,
    },
    Despawn(Entity),
    SetComponent {
        f: fn(&mut World, *mut u8, Entity),
        entity: Entity,
        data_size: usize
    },
    RemoveComponent {
        f: fn(&mut World, Entity),
        entity: Entity,
    },
    SendSignal {
        f: fn(&mut World, *mut u8, Option<Entity>),
        target: Option<Entity>,
        data_size: usize,
    },
    RunSchedule {
        f: fn(&mut World, *mut u8),
        data_size: usize,
    },
    InsertResource {
        f: fn(&mut World, *mut u8),
        data_size: usize,
    },
    RemoveResource {
        f: fn(&mut World),
    },
    AddSystem {
        f: fn(&mut World, *mut u8, *mut u8, SystemId),
        label_data_size: usize,
        system_data_size: usize,
        system_id: SystemId,
    },
    AddObserver {
        f: fn(&mut World, *mut u8, SystemId),
        data_size: usize,
        system_id: SystemId,
    },
    SendEvent {
        f: fn(&mut World, *mut u8),
        data_size: usize,
    },
    RemoveResourceById {
        resource_id: ResourceId,
    },
    AddChild {
        parent: Entity,
        child: Entity,
    },
    RemoveChild {
        parent: Entity,
        child: Entity,
    },
    RemoveChildren {
        entity: Entity
    },
}

impl Commands<'_> {
    #[inline]
    fn copy_data<T>(&mut self, value: T, index: usize) {
        use std::ptr::NonNull;
        let src = NonNull::from(&value).cast::<u8>();
        let dst = unsafe { NonNull::from(&self.queue[0]).cast::<u8>().add(index) };
        unsafe { src.copy_to_nonoverlapping(dst, size_of::<T>()) };
        std::mem::forget(value);
    }

    pub fn spawn<B: ComponentBundle>(&mut self, bundle: B) -> Entity {
        let additional = size_of::<CommandMeta>() + size_of::<B>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);
        let entity = self.entities.spawn();

        let command_meta = CommandMeta::Spawn {
            f: |world, data, entity| {
                let data = data as *mut B;
                let bundle = unsafe { data.read_unaligned() };
                world.spawn_reserved(entity, bundle);
            },
            data_size: size_of::<B>(),
            entity,
        };

        self.copy_data(command_meta, index);
        self.copy_data(bundle, index + size_of::<CommandMeta>());
        entity
    }

    pub fn set_component<C: Component>(&mut self, entity: Entity, component: C) {
        let additional = size_of::<CommandMeta>() + size_of::<C>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);

        let command_meta = CommandMeta::SetComponent {
            f: |world, data, entity| {
                let data = data as *mut C;
                let component = unsafe { data.read_unaligned() };
                world.set_component(entity, component);
            },
            data_size: size_of::<C>(),
            entity
        };

        self.copy_data(command_meta, index);
        self.copy_data(component, index + size_of::<CommandMeta>());
    }

    pub fn despawn(&mut self, entity: Entity) {
        let additional = size_of::<CommandMeta>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);

        let command_meta = CommandMeta::Despawn(entity);

        self.copy_data(command_meta, index);
    }

    pub fn remove_component<C: Component>(&mut self, entity: Entity) {
        let additional = size_of::<CommandMeta>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);

        let command_meta = CommandMeta::RemoveComponent {
            f: |world, entity| {
                world.remove_component::<C>(entity);
            },
            entity,
        };

        self.copy_data(command_meta, index);
    }

    pub fn send_signal<E: Event>(&mut self, event: E, target: Option<Entity>) {
        let additional = size_of::<CommandMeta>() + size_of::<E>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);

        let command_meta = CommandMeta::SendSignal {
            f: |world, data, target| {
                let data = data as *mut E;
                let event = unsafe { data.read_unaligned() };
                world.send_signal_from_system(event, target);
            },
            target,
            data_size: size_of::<E>(),
        };

        self.copy_data(command_meta, index);
        self.copy_data(event, index + size_of::<CommandMeta>());
    }

    pub fn run_schedule<L: ScheduleLabel>(&mut self, label: L) {
        let additional = size_of::<CommandMeta>() + size_of::<L>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);
        
        let command_meta = CommandMeta::RunSchedule {
            f: |world, data| {
                let data = data as *mut L;
                let label = unsafe { data.read_unaligned() };
                world.run_schedule(label);
            },
            data_size: size_of::<L>(),
        };

        self.copy_data(command_meta, index);
        self.copy_data(label, index + size_of::<CommandMeta>());
    }

    pub fn insert_resource<R: Resource>(&mut self, resource: R) {
        let additional = size_of::<CommandMeta>() + size_of::<R>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);

        let command_meta = CommandMeta::InsertResource {
            f: |world, data| {
                let data = data as *mut R;
                let resource = unsafe { data.read_unaligned() };
                world.insert_resource(resource);
            },
            data_size: size_of::<R>(),
        };

        self.copy_data(command_meta, index);
        self.copy_data(resource, index + size_of::<CommandMeta>());
    }

    pub fn remove_resource<R: Resource>(&mut self) {
        let additional = size_of::<CommandMeta>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);

        let command_meta = CommandMeta::RemoveResource {
            f: |world| {
                world.remove_resource::<R>();
            },
        };

        self.copy_data(command_meta, index);
    }

    pub fn add_system<L: ScheduleLabel, ParamIn: SystemInput, S: IntoSystem<ParamIn, ()> + 'static>(&mut self, label: L, system: S) -> SystemId {
        let additional = size_of::<CommandMeta>() + size_of::<L>() + size_of::<S>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);
        let system_id = SystemId::new();
        
        let command_meta = CommandMeta::AddSystem {
            f: |world, label_data, system_data, system_id| {
                let label = unsafe { (label_data as *mut L).read_unaligned() };
                let system = unsafe { (system_data as *mut S).read_unaligned() };
                world.add_system_with_id(label, system, system_id);
            },
            label_data_size: size_of::<L>(),
            system_data_size: size_of::<S>(),
            system_id: system_id.clone(),
        };

        self.copy_data(command_meta, index);
        self.copy_data(label, index + size_of::<CommandMeta>());
        self.copy_data(system, index + size_of::<CommandMeta>() + size_of::<L>());

        system_id
    }

    pub fn add_observer<ParamIn: ObserverInput, S: IntoSystem<ParamIn, SignalInput> + 'static>(&mut self, system: S) -> SystemId {
        let additional = size_of::<CommandMeta>() + size_of::<S>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);
        let system_id = SystemId::new();
        
        let command_meta = CommandMeta::AddObserver {
            f: |world, data, system_id| {
                let system = unsafe { (data as *mut S).read_unaligned() };
                world.add_observer_with_id(system, system_id);
            },
            data_size: size_of::<S>(),
            system_id: system_id.clone(),
        };

        self.copy_data(command_meta, index);
        self.copy_data(system, index + size_of::<CommandMeta>() );

        system_id
    }

    pub fn send_event<E: Event>(&mut self, event: E) {
        let additional = size_of::<CommandMeta>() + size_of::<E>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);

        let command_meta = CommandMeta::SendEvent {
            f: |world, data| {
                let data = data as *mut E;
                let event = unsafe { data.read_unaligned() };
                world.send_event(event);
            },
            data_size: size_of::<E>(),
        };

        self.copy_data(command_meta, index);
        self.copy_data(event, index + size_of::<CommandMeta>());
    }

    pub fn remove_resource_by_id(&mut self, resource_id: ResourceId) {
        let additional = size_of::<CommandMeta>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);
        
        let command_meta = CommandMeta::RemoveResourceById {
            resource_id,
        };

        self.copy_data(command_meta, index);
    }

    pub fn add_child(&mut self, parent: Entity, child: Entity) {
        let additional = size_of::<CommandMeta>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);
        
        let command_meta = CommandMeta::AddChild { parent, child };

        self.copy_data(command_meta, index);
    }

    pub fn remove_child(&mut self, parent: Entity, child: Entity) {
        let additional = size_of::<CommandMeta>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);
        
        let command_meta = CommandMeta::RemoveChild { parent, child };

        self.copy_data(command_meta, index);
    }

    pub fn remove_children(&mut self, entity: Entity) {
        let additional = size_of::<CommandMeta>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);
        
        let command_meta = CommandMeta::RemoveChildren { entity };

        self.copy_data(command_meta, index);
    }

    #[inline]
    unsafe fn read_command_meta(queue: &[u8], index: usize) -> CommandMeta {
        use std::ptr::NonNull;
        let ptr = NonNull::from(&queue[index]).cast::<CommandMeta>();

        unsafe { ptr.read_unaligned() }
    }

    pub(crate) fn process(queue: &mut Vec<u8>, world: &mut World) {
        let len = queue.len();
        let mut cursor = 0;
        while cursor < len {
            let command_meta = unsafe { Self::read_command_meta(queue, cursor) };
            cursor += size_of::<CommandMeta>();
            match command_meta {
                CommandMeta::Spawn { f, data_size, entity } => {
                    let ptr = unsafe { (&mut queue[0] as *mut u8).add(cursor) };
                    (f)(world, ptr, entity);
                    cursor += data_size;
                },
                CommandMeta::Despawn(entity) => {
                    world.despawn(entity);
                },
                CommandMeta::SetComponent { f, entity, data_size } => {
                    let ptr = unsafe { (&mut queue[0] as *mut u8).add(cursor) };
                    (f)(world, ptr, entity);
                    cursor += data_size;
                },
                CommandMeta::RemoveComponent { f, entity } => {
                    (f)(world, entity);
                },
                CommandMeta::SendSignal { f, target, data_size } => {
                    let ptr = unsafe { (&mut queue[0] as *mut u8).add(cursor) };
                    (f)(world, ptr, target);
                    cursor += data_size;
                },
                CommandMeta::RunSchedule { f, data_size } => {
                    let ptr = unsafe { (&mut queue[0] as *mut u8).add(cursor) };
                    (f)(world, ptr);
                    cursor += data_size;
                },
                CommandMeta::InsertResource { f, data_size } => {
                    let ptr = unsafe { (&mut queue[0] as *mut u8).add(cursor) };
                    (f)(world, ptr);
                    cursor += data_size;
                },
                CommandMeta::RemoveResource { f } => {
                    (f)(world);
                },
                CommandMeta::AddSystem { f, label_data_size, system_data_size, system_id } => {
                    let label_data = unsafe { (&mut queue[0] as *mut u8).add(cursor) };
                    let system_data = unsafe { (&mut queue[0] as *mut u8).add(cursor + label_data_size) };
                    (f)(world, label_data, system_data, system_id);
                    cursor += label_data_size + system_data_size;
                },
                CommandMeta::AddObserver { f, data_size, system_id } => {
                    let ptr = unsafe { (&mut queue[0] as *mut u8).add(cursor) };
                    (f)(world, ptr, system_id);
                    cursor += data_size;
                },
                CommandMeta::SendEvent { f, data_size } => {
                    let ptr = unsafe { (&mut queue[0] as *mut u8).add(cursor) };
                    (f)(world, ptr);
                    cursor += data_size;
                },
                CommandMeta::RemoveResourceById { resource_id } => {
                    world.remove_resource_by_id(resource_id);
                },
                CommandMeta::AddChild { parent, child } => {
                    world.add_child(parent, child);
                },
                CommandMeta::RemoveChild { parent, child } => {
                    world.remove_child(parent, child);
                },
                CommandMeta::RemoveChildren { entity } => {
                    world.remove_children(entity);
                },
            }
        }
        queue.clear();
    }

    pub(crate) fn join(&mut self, buffer: &[u8]) {
        self.queue.extend(buffer);
    }

    #[inline]
    pub(crate) const fn new<'a>(buffer: &'a mut Vec<u8>, entities: &'a Entities) -> Commands<'a> {
        Commands { queue: buffer, entities }
    }
}

impl SystemParam for Commands<'_> {
    type Item<'a> = Commands<'a>;
    type State = Vec<u8>;

    fn init_state(_: &mut World, _: &SystemHandle) -> Self::State {
        Vec::new()
    }

    unsafe fn fetch<'a>(world_ptr: WorldPtr<'a>, state: &'a mut Self::State, _: &SystemHandle) -> Self::Item<'a> {
        Commands {
            queue: state,
            entities: &unsafe { world_ptr.as_world() }.entities
        }
    }

    fn after(commands: &mut Commands, state: &mut Self::State) {
        commands.join(state);
        *state = Vec::new();
    }
}
