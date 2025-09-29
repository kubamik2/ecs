use crate::{access::Access, param::SystemParam, signal::Signal, system::{System, SystemFunc, SystemHandle, SYSTEM_IDS, Commands}, world::WorldPtr, Entity, Event, IntoSystem, SystemId, World};
use std::{any::TypeId, collections::HashMap, ptr::NonNull};

#[derive(Default)]
pub struct Observers {
    event_to_systems: HashMap<TypeId, Vec<NonNull<dyn System<Input = SignalInput> + Send + Sync>>>,
    systems: Vec<Box<dyn System<Input = SignalInput> + Send + Sync>>,
}

impl Observers {
    pub(crate) fn add_observer<ParamIn: ObserverInput, S: IntoSystem<ParamIn, SignalInput> + 'static>(&mut self, system: S) -> SystemId {
        let mut system: Box<dyn System<Input = SignalInput> + Send + Sync> = Box::new(system.into_system());
        let event_type_id = *system.signal_access().expect("observer does not have signal access");
        let system_id = system.id();
        self.event_to_systems.entry(event_type_id).or_default().push(NonNull::from(system.as_mut()));
        self.systems.push(system);
        system_id
    }

    pub(crate) fn add_boxed_observer<S: System<Input = SignalInput> + Send + Sync + 'static>(&mut self, mut system: Box<S>) -> SystemId {
        let event_type_id = *system.signal_access().expect("observer does not have signal access");
        let system_id = system.id();
        self.event_to_systems.entry(event_type_id).or_default().push(NonNull::from(system.as_mut()));
        self.systems.push(system);
        system_id
    }

    pub(crate) fn send_signal<E: Event>(&mut self, mut event: E, target: Option<Entity>, mut world_ptr: WorldPtr<'_>) {
        let signal_input = SignalInput {
            event: NonNull::from(&mut event).cast::<()>(),
            target,
        };

        let Some(system_indices) = self.event_to_systems.get(&TypeId::of::<E>()) else { return; }; 
        for mut system_ptr in system_indices.iter().copied() {
            let system = unsafe { system_ptr.as_mut() };
            if !system.id().is_alive() { continue; }
            if !system.is_init() {
                system.init(unsafe { world_ptr.as_world_mut() });
            }
            system.execute(world_ptr, signal_input);
            system.after(unsafe { world_ptr.as_world_mut() }.command_buffer());
        }
        unsafe { world_ptr.as_world_mut() }.process_command_buffer();
    }

    pub(crate) fn remove_dead_observers(&mut self) {
        let mut i = 0;
        let system_ids = SYSTEM_IDS.read().unwrap();
        while i < self.systems.len() {
            let id = self.systems[i].id();
            let event_type_id = self.systems[i].signal_access().expect("observer no signal access");
            if !system_ids.is_alive(id.get()) {
                let system_ptrs = self.event_to_systems.get_mut(event_type_id).expect("dangling event");

                let position = system_ptrs
                    .iter()
                    .position(|p| unsafe { p.as_ref() }.id() == id)
                    .expect("dangling observer");
                system_ptrs.swap_remove(position);

                self.systems.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct SignalInput {
    pub event: NonNull<()>,
    pub target: Option<Entity>,
}

pub trait ObserverInput {}

impl<E: Event> ObserverInput for Signal<'_, E> {}

macro_rules! observer_input_impl {
    ($($param:ident),+) => {
        #[allow(unused_parens)]
        impl<'a, E: Event, $($param: SystemParam),+> ObserverInput for (Signal<'a, E>, $($param),+) {}
    }
}

variadics_please::all_tuples!{observer_input_impl, 1, 31, In}

macro_rules! observer_func_impl {
    ($(($i:tt, $param:ident, $p:ident)),+) => {
        #[allow(unused_parens)]
        impl<'b, E: Event, F, $($param),+> SystemFunc<(Signal<'b, E>, $($param),+), SignalInput> for F where
            F: Send + Sync + 'static,
            for<'a> &'a F: 
                FnMut(Signal<'a, E>, $($param),+) +
                FnMut(Signal<'a, E>, $($param::Item<'a>),+),
            $($param: for<'a> SystemParam + 'static),+
        {
            type State = ($($param::State),+);
            fn run<'a>(&self, world_ptr: WorldPtr<'a>, state: &'a mut Self::State, input: SignalInput, system_meta: &SystemHandle) {
                #[allow(clippy::too_many_arguments)]
                fn call<'a, E: Event, $($param),+>(mut f: impl FnMut(Signal<'a, E>, $($param),+), s: Signal<'a, E>, $($p:$param),+) {
                    f(s, $($p),+);
                }
                unsafe {
                    $(let $p = $param::fetch(world_ptr, &mut state.$i, system_meta);)+
                    let signal = Signal::fetch(world_ptr, input);
                    call(self, signal, $($p),+);
                }
            }
            
            fn join_component_access(world: &mut World, component_access: &mut Access) {
                $($param::join_component_access(world, component_access);)+
            }

            fn join_resource_access(world: &mut World, resource_access: &mut Access) {
                $($param::join_resource_access(world, resource_access);)+
            }

            fn name(&self) -> &'static str {
                std::any::type_name::<F>()
            }

            fn init_state(world: &mut World) -> Self::State {
                ($($param::init_state(world)),+)
            }

            fn signal_access() -> Option<TypeId> {
                Some(TypeId::of::<E>())
            }

            fn after<'state>(mut commands: Commands<'state>, state: &'state mut Self::State) {
                $($param::after(&mut commands, &mut state.$i);)+
            }
        }
    }
}

variadics_please::all_tuples_enumerated!{observer_func_impl, 2, 31, In, p}

impl<E: Event, F, In> SystemFunc<(Signal<'_, E>, In), SignalInput> for F where 
    F: Send + Sync + 'static,
    for<'a> &'a F:
        FnMut(Signal<'a, E>, In) +
        FnMut(Signal<'a, E>, In::Item<'a>),
    In: for<'a> SystemParam + 'static,
{
    type State = In::State;
    fn run<'a>(&self, world_ptr: WorldPtr<'a>, state: &'a mut Self::State, input: SignalInput, system_meta: &SystemHandle) {
        fn call<'a, E: Event, In>(mut f: impl FnMut(Signal<'a, E>, In), s: Signal<'a, E>, p: In) {
            f(s, p)
        }
        unsafe {
            let p = In::fetch(world_ptr, state, system_meta);
            let signal = Signal::fetch(world_ptr, input);
            call(self, signal, p);
        }
    }

    fn join_component_access(world: &mut World, component_access: &mut Access) {
        In::join_component_access(world, component_access);
    }

    fn join_resource_access(world: &mut World, resource_access: &mut Access) {
        In::join_resource_access(world, resource_access);
    }

    fn name(&self) -> &'static str {
        std::any::type_name::<F>()
    }

    fn init_state(world: &mut World) -> Self::State {
        In::init_state(world)
    }

    fn signal_access() -> Option<TypeId> {
        Some(TypeId::of::<E>())
    }

    fn after<'state>(mut commands: Commands<'state>, state: &'state mut Self::State) {
        In::after(&mut commands, state);
    }
}

impl<E: Event, F> SystemFunc<Signal<'_, E>, SignalInput> for F where 
    F: Send + Sync + 'static,
    for<'a> &'a F: FnMut(Signal<'a, E>)
{
    type State = ();
    fn run<'a>(&self, world_ptr: WorldPtr<'a>, _: &'a mut Self::State, input: SignalInput, _: &SystemHandle) {
        fn call<'a, E: Event>(mut f: impl FnMut(Signal<'a, E>), s: Signal<'a, E>) {
            f(s)
        }
        let signal = unsafe { Signal::fetch(world_ptr, input) };
        call(self, signal);
    }

    fn join_component_access(_: &mut World, _: &mut Access) {}

    fn join_resource_access(_: &mut World, _: &mut Access) {}

    fn name(&self) -> &'static str {
        std::any::type_name::<F>()
    }

    fn init_state(_: &mut World) -> Self::State {}

    fn signal_access() -> Option<TypeId> {
        Some(TypeId::of::<E>())
    }

    fn after(_: Commands, _: &mut Self::State) {}
}
