use crate::{Entity, IntoSystem, SystemId, World, access::AccessBuilder, param::{SystemParam, SystemParamError}, system::{Commands, System, SystemFunc, SystemHandle, SystemOutput, error::InternalSystemError}, trigger::Trigger, world::WorldPtr};
use std::{any::TypeId, collections::HashMap, ptr::NonNull};

#[derive(Default)]
pub struct Observers {
    event_to_systems: HashMap<TypeId, Vec<NonNull<dyn System<Input = TriggerInput> + Send + Sync>>>,
    systems: Vec<Box<dyn System<Input = TriggerInput> + Send + Sync>>,
}

impl Observers {
    pub(crate) fn add_observer<ParamIn: ObserverInput, Output: SystemOutput, S: IntoSystem<ParamIn, TriggerInput, Output> + 'static>(&mut self, system: S) -> SystemId {
        let mut system: Box<dyn System<Input = TriggerInput> + Send + Sync> = Box::new(system.into_system());
        let event_type_id = *system.trigger_access().expect("observer does not have trigger access");
        let system_id = system.id().clone();
        self.event_to_systems.entry(event_type_id).or_default().push(NonNull::from(system.as_mut()));
        self.systems.push(system);
        system_id
    }

    pub(crate) fn add_boxed_observer<S: System<Input = TriggerInput> + Send + Sync + 'static>(&mut self, mut system: Box<S>) -> SystemId {
        let event_type_id = *system.trigger_access().expect("observer does not have trigger access");
        let system_id = system.id().clone();
        self.event_to_systems.entry(event_type_id).or_default().push(NonNull::from(system.as_mut()));
        self.systems.push(system);
        system_id
    }

    pub(crate) fn trigger<E: Send + Sync + 'static>(&mut self, mut event: E, target: Option<Entity>, mut world_ptr: WorldPtr<'_>) -> Result<(), InternalSystemError> {
        let trigger_input = TriggerInput {
            event: NonNull::from(&mut event).cast::<()>(),
            target,
        };

        let Some(system_indices) = self.event_to_systems.get_mut(&TypeId::of::<E>()) else { return Ok(()); }; 
        for mut system_ptr in system_indices.iter().copied() {
            let system = unsafe { system_ptr.as_mut() };
            if !system.id().is_alive() { continue; }
            if !system.is_init() {
                system.init(unsafe { world_ptr.as_world_mut() })?;
            }
            system.execute(world_ptr, trigger_input);
            system.after(unsafe { world_ptr.as_world_mut() }.command_buffer());
        }
        unsafe { world_ptr.as_world_mut() }.process_command_buffer();
        Ok(())
    }

    pub(crate) fn remove_dead_observers(&mut self) {
        let mut i = 0;
        while i < self.systems.len() {
            let id = self.systems[i].id();
            let event_type_id = self.systems[i].trigger_access().expect("observer no trigger access");
            if !id.is_alive() {
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
pub struct TriggerInput {
    pub event: NonNull<()>,
    pub target: Option<Entity>,
}

pub trait ObserverInput {}

impl<E: Send + Sync + 'static> ObserverInput for Trigger<'_, E> {}

macro_rules! observer_input_impl {
    ($($param:ident),+) => {
        #[allow(unused_parens)]
        impl<'a, E: Send + Sync + 'static, $($param: SystemParam),+> ObserverInput for (Trigger<'a, E>, $($param),+) {}
    }
}

variadics_please::all_tuples!{observer_input_impl, 1, 31, In}

macro_rules! observer_func_impl {
    ($(($i:tt, $param:ident, $p:ident)),+) => {
        #[allow(unused_parens)]
        impl<'b, E: Send + Sync + 'static, F, $($param),+, Output> SystemFunc<(Trigger<'b, E>, $($param),+), TriggerInput, Output> for F where
            F: Send + Sync + 'static,
            for<'a> &'a F: 
                FnMut(Trigger<'a, E>, $($param),+) -> Output +
                FnMut(Trigger<'a, E>, $($param::Item<'a>),+) -> Output,
            $($param: for<'a> SystemParam + 'static),+
        {
            type State = ($($param::State),+);
            fn run<'a>(&self, world_ptr: WorldPtr<'a>, state: &'a mut Self::State, input: TriggerInput, system_meta: SystemHandle) -> Output {
                #[allow(clippy::too_many_arguments)]
                fn call<'a, E: Send + Sync + 'static, $($param),+, Output>(mut f: impl FnMut(Trigger<'a, E>, $($param),+) -> Output, s: Trigger<'a, E>, $($p:$param),+) -> Output {
                    f(s, $($p),+)
                }
                unsafe {
                    $(let $p = $param::fetch(world_ptr, &mut state.$i, &system_meta);)+
                    let trigger = Trigger::fetch(world_ptr, input);
                    call(self, trigger, $($p),+)
                }
            }

            fn join_access(world: &mut World, access: &mut AccessBuilder) -> Result<(), SystemParamError> {
                $($param::join_access(world, access)?;)+
                Ok(())
            }

            fn name(&self) -> &'static str {
                std::any::type_name::<F>()
            }

            fn init_state(world: &mut World, system_handle: SystemHandle) -> Result<Self::State, SystemParamError> {
                Ok(($($param::init_state(world, &system_handle)?),+))
            }

            fn trigger_access() -> Option<TypeId> {
                Some(TypeId::of::<E>())
            }

            fn after<'state>(commands: &mut Commands<'state>, state: &'state mut Self::State) {
                $($param::after(commands, &mut state.$i);)+
            }
        }
    }
}

variadics_please::all_tuples_enumerated!{observer_func_impl, 2, 31, In, p}

impl<E: Send + Sync + 'static, F, In, Output> SystemFunc<(Trigger<'_, E>, In), TriggerInput, Output> for F where 
    F: Send + Sync + 'static,
    for<'a> &'a F:
        FnMut(Trigger<'a, E>, In) -> Output +
        FnMut(Trigger<'a, E>, In::Item<'a>) -> Output,
    In: for<'a> SystemParam + 'static,
{
    type State = In::State;
    fn run<'a>(&self, world_ptr: WorldPtr<'a>, state: &'a mut Self::State, input: TriggerInput, system_meta: SystemHandle) -> Output {
        fn call<'a, E: Send + Sync + 'static, In, Output>(mut f: impl FnMut(Trigger<'a, E>, In) -> Output, s: Trigger<'a, E>, p: In) -> Output {
            f(s, p)
        }
        unsafe {
            let p = In::fetch(world_ptr, state, &system_meta);
            let trigger = Trigger::fetch(world_ptr, input);
            call(self, trigger, p)
        }
    }

    fn join_access(world: &mut World, access: &mut AccessBuilder) -> Result<(), SystemParamError> {
        In::join_access(world, access)
    }

    fn name(&self) -> &'static str {
        std::any::type_name::<F>()
    }

    fn init_state(world: &mut World, system_handle: SystemHandle) -> Result<Self::State, SystemParamError> {
        In::init_state(world, &system_handle)
    }

    fn trigger_access() -> Option<TypeId> {
        Some(TypeId::of::<E>())
    }

    fn after<'state>(commands: &mut Commands<'state>, state: &'state mut Self::State) {
        In::after(commands, state);
    }
}

impl<E: Send + Sync + 'static, F, Output> SystemFunc<Trigger<'_, E>, TriggerInput, Output> for F where 
    F: Send + Sync + 'static,
    for<'a> &'a F: FnMut(Trigger<'a, E>) -> Output
{
    type State = ();
    fn run<'a>(&self, world_ptr: WorldPtr<'a>, _: &'a mut Self::State, input: TriggerInput, _: SystemHandle) -> Output {
        fn call<'a, E: Send + Sync + 'static, Output>(mut f: impl FnMut(Trigger<'a, E>) -> Output, s: Trigger<'a, E>) -> Output {
            f(s)
        }
        let trigger = unsafe { Trigger::fetch(world_ptr, input) };
        call(self, trigger)
    }

    fn join_access(_: &mut World, _: &mut AccessBuilder) -> Result<(), SystemParamError> { Ok(()) }

    fn name(&self) -> &'static str {
        std::any::type_name::<F>()
    }

    fn init_state(_: &mut World, _: SystemHandle) -> Result<Self::State, SystemParamError> { Ok(()) }

    fn trigger_access() -> Option<TypeId> {
        Some(TypeId::of::<E>())
    }

    fn after(_: &mut Commands, _: &mut Self::State) {}
}
