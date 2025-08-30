use std::{any::{Any, TypeId}, cell::SyncUnsafeCell, collections::HashMap, hash::Hash};

use crate::{access::Access, system::{IntoSystem, System, SystemInput, SystemValidationError}, world::{World, WorldId, WorldPtr}, Resource};

const PARALLEL_EXECUTION_THRESHOLD: usize = 4;

#[derive(Default)]
pub struct Schedule {
    linked_world: Option<WorldId>,
    systems: Vec<SyncUnsafeCell<Box<dyn System<Input = ()> + Send + Sync>>>,
    // parallel_exeuction_queue: Vec<Vec<*mut Box<dyn System<Input = ()> + Send + Sync>>>,
    parallel_execution_queue: Vec<ParallelBucket>,
    init_queue: Vec<usize>,
}

impl Schedule {

    pub fn add_system<ParamInput: SystemInput, S: IntoSystem<ParamInput, ()> + 'static>(&mut self, system: S) -> Result<(), SystemValidationError> {
        let system: Box<dyn System<Input = ()> + Send + Sync> = Box::new(system.into_system());
        system.validate()?;
        self.init_queue.push(self.init_queue.len());
        let system_index = self.systems.len();

        // add system to parallel_exeuction_queue
        let compatible_bucket = self.parallel_execution_queue
            .iter_mut()
            .find(|bucket| {
                bucket.is_system_compatible(system.as_ref())
            });
        match compatible_bucket {
            Some(bucket) => {
                bucket.add_system(system.as_ref(), system_index);
            },
            None => {
                let mut bucket = ParallelBucket::default();
                bucket.add_system(system.as_ref(), system_index);
                self.parallel_execution_queue.push(bucket);
            }
        }

        self.systems.push(SyncUnsafeCell::new(system));
        Ok(())
    }

    pub(crate) fn execute(&mut self, mut world_ptr: WorldPtr<'_>) {
        for bucket in &self.parallel_execution_queue {
            if bucket.should_run_paralell {
                unsafe { world_ptr.as_world() }.thread_pool.in_place_scope(|scope| {
                    for system_index in bucket.systems.iter().copied() {
                        let system = unsafe { self.systems[system_index].get().as_mut().unwrap() };
                        scope.spawn(|_| {
                            system.execute(world_ptr, ());
                        });
                    }
                });
            } else {
                for system_index in bucket.systems.iter().copied() {
                    let system = self.systems[system_index].get_mut();
                    system.execute(world_ptr, ());
                }
            }
        }

        let world = unsafe { world_ptr.as_world_mut() };
        for system in self.systems.iter_mut() {
            let system = system.get_mut();
            system.after(world);
        }
    }

    pub fn run(&mut self, world: &mut World) {
        let world_id = self.linked_world.unwrap_or(world.id());
        assert!(world.id() == world_id, "initialized schedule ran in a different world");

        while let Some(i) = self.init_queue.pop() {
            self.systems[i].get_mut().init_state(world);
        }

        self.execute(world.world_ptr_mut());
    }
}

#[derive(Default)]
pub struct ParallelBucket {
    joined_component_access: Access,
    joined_resource_access: Access,
    systems: Vec<usize>,
    should_run_paralell: bool,
}

impl ParallelBucket {
    fn is_system_compatible(&self, system: &dyn System<Input = ()>) -> bool {
        self.joined_component_access.is_compatible(system.component_access()) &&
        self.joined_resource_access.is_compatible(system.resource_access())
    }

    fn add_system(&mut self, system: &dyn System<Input = ()>, system_index: usize) {
        self.joined_component_access.join(system.component_access());
        self.joined_resource_access.join(system.resource_access());
        self.systems.push(system_index);
        self.should_run_paralell |= self.systems.len() >= PARALLEL_EXECUTION_THRESHOLD;
    }
}

pub trait ScheduleLabel: 'static + PartialEq + Eq + Hash {}

#[derive(Default)]
pub struct Schedules(HashMap<TypeId, Box<dyn Any>>);

unsafe impl Send for Schedules {}
unsafe impl Sync for Schedules {}

impl Resource for Schedules {}

impl Schedules {
    pub fn get<L: ScheduleLabel>(&self, label: &L) -> Option<&Schedule> {
        let boxed_type_schedules = self.0.get(&TypeId::of::<L>())?;
        let type_schedules = unsafe { boxed_type_schedules.downcast_ref_unchecked::<HashMap<L, Schedule>>() };
        type_schedules.get(label)
    }

    pub fn get_mut<L: ScheduleLabel>(&mut self, label: &L) -> Option<&mut Schedule> {
        let boxed_type_schedules = self.0.get_mut(&TypeId::of::<L>())?;
        let type_schedules = unsafe { boxed_type_schedules.downcast_mut_unchecked::<HashMap<L, Schedule>>() };
        type_schedules.get_mut(label)
    }

    pub fn insert<L : ScheduleLabel>(&mut self, label: L, schedule: Schedule) {
        let boxed_type_schedules = self.0
            .entry(TypeId::of::<L>())
            .or_insert(Box::new(HashMap::<L, Schedule>::default()));
        let type_schedules = unsafe { boxed_type_schedules.downcast_mut_unchecked::<HashMap<L, Schedule>>() };
        type_schedules.insert(label, schedule);
    }
}
