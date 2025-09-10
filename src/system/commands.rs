use crate::{entity::EntityBundle, param::SystemParam, world::WorldPtr, Component, Entity, Event, ScheduleLabel, World};

use super::{SystemHandle, SystemId, SYSTEM_IDS};

pub struct Commands<'a> {
    queue: &'a mut Vec<u8>,
}

enum CommandMeta {
    Spawn {
        f: fn(&mut World, *mut u8),
        data_size: usize,
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
    RemoveSystem {
        id: SystemId,
    },
    RunSchedule {
        f: fn(&mut World, *mut u8),
        data_size: usize,
    }
}

impl Commands<'_> {
    #[inline]
    fn copy_data<T>(&mut self, value: &T, index: usize) {
        use std::ptr::NonNull;
        let src = NonNull::from(value).cast::<u8>();
        let dst = unsafe { NonNull::from(&self.queue[0]).cast::<u8>().add(index) };
        unsafe { src.copy_to_nonoverlapping(dst, size_of::<T>()) };
    }

    pub fn spawn<B: EntityBundle>(&mut self, bundle: B) {
        let additional = size_of::<CommandMeta>() + size_of::<B>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);

        let command_meta = CommandMeta::Spawn {
            f: |world, data| {
                let data = data as *mut B;
                let bundle = unsafe { data.read_unaligned() };
                world.spawn(bundle);
            },
            data_size: size_of::<B>(),
        };

        self.copy_data(&command_meta, index);
        self.copy_data(&bundle, index + size_of::<CommandMeta>());

        std::mem::forget(bundle);
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

        self.copy_data(&command_meta, index);
        self.copy_data(&component, index + size_of::<CommandMeta>());

        std::mem::forget(component);
    }

    pub fn despawn(&mut self, entity: Entity) {
        let additional = size_of::<CommandMeta>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);

        let command_meta = CommandMeta::Despawn(entity);

        self.copy_data(&command_meta, index);
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

        self.copy_data(&command_meta, index);
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

        self.copy_data(&command_meta, index);
        self.copy_data(&event, index + size_of::<CommandMeta>());

        std::mem::forget(event);
    }

    pub fn remove_system(&mut self, id: SystemId) {
        let additional = size_of::<CommandMeta>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);

        let command_meta = CommandMeta::RemoveSystem {
            id,
        };

        self.copy_data(&command_meta, index);
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

        self.copy_data(&command_meta, index);
        self.copy_data(&label, index + size_of::<L>());
    }

    unsafe fn read_command_meta(&self, index: usize) -> CommandMeta {
        use std::ptr::NonNull;
        let ptr = NonNull::from(&self.queue[index]).cast::<CommandMeta>();

        unsafe { ptr.read_unaligned() }
    }

    pub(crate) fn process(&mut self, world: &mut World) {
        let len = self.queue.len();
        let mut cursor = 0;
        while cursor < len {
            let command_meta = unsafe { self.read_command_meta(cursor) };
            cursor += size_of::<CommandMeta>();
            match command_meta {
                CommandMeta::Spawn { f, data_size } => {
                    let ptr = unsafe { (&mut self.queue[0] as *mut u8).add(cursor) };
                    (f)(world, ptr);
                    cursor += data_size;
                },
                CommandMeta::Despawn(entity) => {
                    world.despawn(entity);
                },
                CommandMeta::SetComponent { f, entity, data_size } => {
                    let ptr = unsafe { (&mut self.queue[0] as *mut u8).add(cursor) };
                    (f)(world, ptr, entity);
                    cursor += data_size;
                },
                CommandMeta::RemoveComponent { f, entity } => {
                    (f)(world, entity);
                },
                CommandMeta::SendSignal { f, target, data_size } => {
                    let ptr = unsafe { (&mut self.queue[0] as *mut u8).add(cursor) };
                    (f)(world, ptr, target);
                    cursor += data_size;
                },
                CommandMeta::RemoveSystem { id } => {
                    SYSTEM_IDS.write().unwrap().despawn(id.get());
                },
                CommandMeta::RunSchedule { f, data_size } => {
                    let ptr = unsafe { (&mut self.queue[0] as *mut u8).add(cursor) };
                    (f)(world, ptr);
                    cursor += data_size;
                }
            }
        }
        self.queue.clear();
    }
}

impl SystemParam for Commands<'_> {
    type Item<'a> = Commands<'a>;
    type State = Vec<u8>;

    fn init_state(_: &mut World) -> Self::State {
        Vec::new()
    }

    unsafe fn fetch<'a>(_: WorldPtr<'a>, state: &'a mut Self::State, _: &SystemHandle) -> Self::Item<'a> {
        Commands {
            queue: state,
        }
    }

    fn after(world: &mut World, state: &mut Self::State, _: &mut SystemHandle) {
        let mut commands = Commands {
            queue: state,
        };
        commands.process(world);
    }
}
