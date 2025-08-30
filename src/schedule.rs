use std::{any::{Any, TypeId}, cell::SyncUnsafeCell, collections::HashMap, hash::Hash};

use crate::{system::{IntoSystem, System, SystemInput, SystemValidationError}, world::{World, WorldId, WorldPtr}, Resource};

#[derive(Default)]
pub struct Schedule {
    linked_world: Option<WorldId>,
    systems: Vec<SyncUnsafeCell<Box<dyn System<Input = ()> + Send + Sync>>>,
    parallel_exeuction_queue: Vec<Vec<*mut Box<dyn System<Input = ()> + Send + Sync>>>,
    init_queue: Vec<usize>,
}

impl Schedule {
    const PARALLEL_EXECUTION_THRESHOLD: usize = 4;

    pub fn add_system<ParamInput: SystemInput, S: IntoSystem<ParamInput, ()> + 'static>(&mut self, system: S) -> Result<(), SystemValidationError> {
        let system: Box<dyn System<Input = ()> + Send + Sync> = Box::new(system.into_system());
        system.validate()?;
        self.init_queue.push(self.init_queue.len());
        self.systems.push(SyncUnsafeCell::new(system));
        self.update_parallel_execution_queue();
        Ok(())
    }

    pub(crate) fn execute(&mut self, mut world_ptr: WorldPtr<'_>) {
        for pack in &self.parallel_exeuction_queue {
            if pack.len() > Self::PARALLEL_EXECUTION_THRESHOLD {
                unsafe { world_ptr.as_world() }.thread_pool.in_place_scope(|scope| {
                    for system in pack {
                        let system = unsafe { system.as_mut().unwrap() };
                        scope.spawn(|_| {
                            system.execute(world_ptr, ());
                        });
                    }
                });
            } else {
                for system in pack {
                    let system = unsafe { system.as_mut().unwrap() };
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
        if let Some(world_id) = self.linked_world {
            assert!(world.id() == world_id, "initialized schedule ran in a different world");
        }
        while let Some(i) = self.init_queue.pop() {
            self.systems[i].get_mut().init_state(world);
        }

        self.execute(world.world_ptr_mut());
    }

    fn update_parallel_execution_queue(&mut self) {
        let mut parallel_exeuction_queue = vec![];
        let mut added_systems = vec![false; self.systems.len()];

        // TODO
        // O(n^2) might want to improve this, although there shouldn't be
        // that many systems for this to be a major slowdown
        for i in 0..self.systems.len() {
            if added_systems[i] { continue; }
            added_systems[i] = true;
            let mut systems = vec![self.systems[i].get()];
            let system = unsafe { self.systems[i].get().as_ref().unwrap() };
            let mut joined_component_access = system.component_access().clone();
            let mut joined_resource_access = system.resource_access().clone();


            for j in i+1..self.systems.len() {
                if added_systems[j] { continue; }
                let other_system = unsafe { self.systems[j].get().as_ref().unwrap() };

                if other_system.is_comp(&joined_component_access, &joined_resource_access) {
                    added_systems[j] = true;
                    let component_access = other_system.component_access();
                    let resource_access = other_system.resource_access();

                    joined_component_access.join(component_access);
                    joined_resource_access.join(resource_access);

                    systems.push(self.systems[j].get());
                }
            }

            parallel_exeuction_queue.push(systems);
        }

        self.parallel_exeuction_queue = parallel_exeuction_queue;
    }
}

pub trait ScheduleLabel: 'static + PartialEq + Eq + Hash {
}

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
