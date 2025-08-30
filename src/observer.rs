use crate::{access::Access, param::SystemParam, signal::Signal, system::{System, SystemFunc}, world::WorldPtr, Entity, Event, Resource, World};
use std::{any::TypeId, collections::HashMap, ptr::NonNull};

#[derive(Default)]
pub struct Observers {
    event_to_systems: HashMap<TypeId, Vec<usize>>,
    systems: Vec<Box<dyn System<Input = SignalInput> + Send + Sync>>,
}

impl Resource for Observers {}

impl Observers {
    pub(crate) fn add_boxed_observer(&mut self, system: Box<dyn System<Input = SignalInput> + Send + Sync>) {
        let system_index = self.systems.len();
        let event_type_id = *system.signal_access().expect("observer does not have signal access");
        self.event_to_systems.entry(event_type_id).or_default().push(system_index);
        self.systems.push(system);

    }

    pub(crate) fn send_signal<E: Event>(&mut self, mut event: E, target: Option<Entity>, mut world_ptr: WorldPtr<'_>) {
        let signal_input = SignalInput {
            event: NonNull::from(&mut event).cast::<()>(),
            target,
        };

        let Some(system_indices) = self.event_to_systems.get(&TypeId::of::<E>()) else { return; }; 
        for system_index in system_indices.iter().copied() {
            let system = &mut self.systems[system_index];
            system.execute(world_ptr, signal_input);
            system.after(unsafe { world_ptr.as_world_mut() });
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
            fn run<'a>(&self, world_ptr: WorldPtr<'a>, state: &'a mut Self::State, input: SignalInput) {
                #[allow(clippy::too_many_arguments)]
                fn call<'a, E: Event, $($param),+>(mut f: impl FnMut(Signal<'a, E>, $($param),+), s: Signal<'a, E>, $($p:$param),+) {
                    f(s, $($p),+);
                }
                unsafe {
                    $(let $p = $param::fetch(world_ptr, &mut state.$i);)+
                    let signal = Signal::fetch(world_ptr, input);
                    call(self, signal, $($p),+);
                }
            }
            
            fn join_component_access(component_access: &mut Access) {
                $($param::join_component_access(component_access);)+
            }

            fn join_resource_access(resource_access: &mut Access) {
                $($param::join_resource_access(resource_access);)+
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

            fn after(world: &mut World, state: &mut Self::State) {
                $($param::after(world, &mut state.$i);)+
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
    fn run<'a>(&self, world_ptr: WorldPtr<'a>, state: &'a mut Self::State, input: SignalInput) {
        fn call<'a, E: Event, In>(mut f: impl FnMut(Signal<'a, E>, In), s: Signal<'a, E>, p: In) {
            f(s, p)
        }
        unsafe {
            let p = In::fetch(world_ptr, state);
            let signal = Signal::fetch(world_ptr, input);
            call(self, signal, p);
        }
    }

    fn join_component_access(component_access: &mut Access) {
        In::join_component_access(component_access);
    }

    fn join_resource_access(resource_access: &mut Access) {
        In::join_resource_access(resource_access);
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

    fn after(world: &mut World, state: &mut Self::State) {
        In::after(world, state);
    }
}

impl<E: Event, F> SystemFunc<Signal<'_, E>, SignalInput> for F where 
    F: Send + Sync + 'static,
    for<'a> &'a F: FnMut(Signal<'a, E>)
{
    type State = ();
    fn run<'a>(&self, world_ptr: WorldPtr<'a>, _: &'a mut Self::State, input: SignalInput) {
        fn call<'a, E: Event>(mut f: impl FnMut(Signal<'a, E>), s: Signal<'a, E>) {
            f(s)
        }
        let signal = unsafe { Signal::fetch(world_ptr, input) };
        call(self, signal);
    }

    fn join_component_access(_: &mut Access) {}

    fn join_resource_access(_: &mut Access) {}

    fn name(&self) -> &'static str {
        std::any::type_name::<F>()
    }

    fn init_state(_: &mut World) -> Self::State {}

    fn signal_access() -> Option<TypeId> {
        Some(TypeId::of::<E>())
    }

    fn after(_: &mut World, _: &mut Self::State) {}
}
