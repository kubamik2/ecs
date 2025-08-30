use fixedbitset::FixedBitSet;

use crate::{access::Access, param::SystemParam, signal::Signal, storage::sparse_set::SparseSet, system::{System, SystemFunc}, world::WorldPtr, Event, Resource, World};
use std::{any::TypeId, cell::SyncUnsafeCell, collections::{HashMap, VecDeque}};

#[derive(Default)]
pub struct Observers {
    event_to_systems: HashMap<TypeId, Vec<usize>>,
    systems: Vec<SystemRecord>,

    parallel_exeuction_buckets: Vec<Bucket>, // each bucket contains systems that can be run in parallell
    is_system_queried: FixedBitSet,
    queried_buckets: SparseSet<usize>,

    system_inputs: Vec<Option<SignalInput>>,
    queued_system_inputs: VecDeque<(usize, SignalInput)>,

    pub(crate) clear_signal_queues: Vec<fn(&mut World)>,
}

impl Resource for Observers {}

pub struct SystemRecord {
    system: SyncUnsafeCell<Box<dyn System<Input = SignalInput> + Send + Sync>>,
    bucket: usize,
}

pub struct Bucket {
    component_access: Access,
    resource_access: Access,
    systems: Vec<usize>,
}

impl Observers {
    const PARALLEL_EXECUTION_THRESHOLD: usize = 4;

    pub(crate) fn add_boxed_observer(&mut self, system: Box<dyn System<Input = SignalInput> + Send + Sync>) {
        let system_index = self.systems.len();
        let event_type_id = *system.signal_access().unwrap();
        self.event_to_systems.entry(event_type_id).or_default().push(system_index);
        self.system_inputs.push(None);
        let bucket = self.get_compatible_bucket_index(system.as_ref(), system_index);
        self.systems.push(SystemRecord {
            system: SyncUnsafeCell::new(system),
            bucket,
        });
        self.is_system_queried.grow(self.systems.len());
    }

    fn get_compatible_bucket_index(&mut self, system: &(dyn System<Input = SignalInput> + Send + Sync), system_index: usize) -> usize {
        for (i, bucket) in self.parallel_exeuction_buckets.iter_mut().enumerate() {
            if !system.is_comp(&bucket.component_access, &bucket.resource_access) { continue; }
            bucket.component_access.join(system.component_access());
            bucket.resource_access.join(system.resource_access());
            bucket.systems.push(system_index);
            return i;
        }
        self.parallel_exeuction_buckets.push(Bucket {
            component_access: system.component_access().clone(),
            resource_access: system.resource_access().clone(),
            systems: vec![system_index],
        });
        self.parallel_exeuction_buckets.len()-1
    }

    pub(crate) fn send_signal<E: Event>(&mut self, signal_index: usize) {
        let Some(queried_systems) = self.event_to_systems.get(&TypeId::of::<E>()) else { return; };

        // set input and mark systems as queried
        for system_index in queried_systems.iter().copied() {
            let SystemRecord { system: _, bucket } = &mut self.systems[system_index];
            if self.system_inputs[system_index].is_some() {
                self.queued_system_inputs.push_back((system_index, SignalInput { signal_index }));
            } else {
                self.is_system_queried.set(system_index, true);
                self.queried_buckets.insert(*bucket, *bucket);
                self.system_inputs[system_index] = Some(SignalInput { signal_index });
            }
        }
    }

    pub(crate) fn execute_queried_systems(&mut self, mut world_ptr: WorldPtr<'_>) {
        let world = unsafe { world_ptr.as_world_mut() };
        for bucket in self.queried_buckets.iter().copied() {
            let queried_systems_in_bucket = self.parallel_exeuction_buckets[bucket].systems.iter().copied().filter(|i| self.is_system_queried[*i]).count();
            if queried_systems_in_bucket > Self::PARALLEL_EXECUTION_THRESHOLD {
                world.thread_pool.scope(|scope| {
                    for system_index in self.parallel_exeuction_buckets[bucket].systems.iter().copied() {
                        if !self.is_system_queried[system_index] { continue; }
                        let Some(input) = self.system_inputs[system_index].take() else { panic!("observer executed without input"); };
                        let system = unsafe { self.systems[system_index].system.get().as_mut().unwrap() };
                        scope.spawn(move |_| {
                            system.execute(world_ptr, input);
                        });
                    }
                });
            } else {
                for system_index in self.parallel_exeuction_buckets[bucket].systems.iter().copied() {
                    if !self.is_system_queried[system_index] { continue; }
                    let Some(input) = self.system_inputs[system_index].take() else { panic!("observer executed without input"); };
                    let system = unsafe { self.systems[system_index].system.get().as_mut().unwrap() };
                    system.execute(world_ptr, input);
                }
            }
        }
        for bucket in self.queried_buckets.iter().copied() {
            for system_index in self.parallel_exeuction_buckets[bucket].systems.iter().copied() {
                if !self.is_system_queried[system_index] { continue; }
                let system = self.systems[system_index].system.get_mut();
                system.after(world);
            }
        }
        self.is_system_queried.clear();
        self.queried_buckets.clear();

        if self.queued_system_inputs.is_empty() {
            for clear in self.clear_signal_queues.iter() {
                (clear)(unsafe { world_ptr.as_world_mut() });
            }
            return;
        }

        for _ in 0..self.queued_system_inputs.len() {
            let (system_index, input) = self.queued_system_inputs.pop_front().unwrap(); 
            self.is_system_queried.set(system_index, true);
            let SystemRecord { system: _, bucket } = &mut self.systems[system_index];
            if self.system_inputs[system_index].is_some() {
                self.queued_system_inputs.push_back((system_index, input));
            } else {
                self.is_system_queried.set(system_index, true);
                self.queried_buckets.insert(*bucket, *bucket);
                self.system_inputs[system_index] = Some(input);
            }
            self.queried_buckets.insert(*bucket, *bucket);
        }
        self.execute_queried_systems(world_ptr);
    }

    pub(crate) fn is_pending(&self) -> bool {
        self.queried_buckets.len() > 0
    }
}

#[derive(Clone, Copy)]
pub struct SignalInput {
    signal_index: usize,
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
                    let signal = Signal::fetch(world_ptr, input.signal_index);
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
            let signal = Signal::fetch(world_ptr, input.signal_index);
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
        let signal = unsafe { Signal::fetch(world_ptr, input.signal_index) };
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
