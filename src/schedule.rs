use std::{any::{Any, TypeId}, collections::HashMap, hash::Hash};

use crate::{system::{IntoSystem, System, SystemInput, SystemValidationError}, ECSId, Resource, ECS};

#[derive(Default)]
pub struct Schedule {
    linked_ecs: Option<ECSId>,
    systems: Vec<Box<dyn System + Send + Sync>>,
    parallel_exeuction_queue: Vec<Vec<usize>>,
    init_queue: Vec<usize>,
}

impl Schedule {
    const PARALLEL_EXECUTION_THRESHOLD: usize = 4;

    pub fn add_system<In: SystemInput, T: IntoSystem<In> + 'static>(&mut self, system: T) -> Result<(), SystemValidationError> {
        let system = system.into_system();
        system.validate()?;
        self.init_queue.push(self.init_queue.len());
        self.systems.push(system);
        self.update_parallel_execution_queue();
        Ok(())
    }

    pub(crate) fn execute(&mut self, ecs: &ECS) {
        for pack in &self.parallel_exeuction_queue {
            if pack.len() > Self::PARALLEL_EXECUTION_THRESHOLD {
                ecs.thread_pool.in_place_scope(|scope| {
                    self.systems.iter_mut().for_each(|system| {
                        scope.spawn(|_| {
                            system.execute(ecs);
                        });
                    });
                });
            } else {
                self.systems.iter_mut().for_each(|system| {
                    system.execute(ecs);
                });
            }
        }
    }

    pub fn run(&mut self, ecs: &mut ECS) {
        if let Some(ecs_id) = self.linked_ecs {
            assert!(ecs.id == ecs_id, "initialized schedule ran in a different ecs");
        }
        while let Some(i) = self.init_queue.pop() {
            self.systems[i].init_state(ecs);
        }
        self.execute(ecs);
        ecs.handle_commands();
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
            let mut systems = vec![i];
            let system = self.systems[i].as_ref();
            let mut joined_component_access = system.component_access().clone();
            let mut joined_resource_access = system.resource_access().clone();


            for j in i+1..self.systems.len() {
                if added_systems[j] { continue; }
                let other_system = self.systems[j].as_ref();

                if other_system.is_comp(&joined_component_access, &joined_resource_access) {
                    added_systems[j] = true;
                    let component_access = other_system.component_access();
                    let resource_access = other_system.resource_access();

                    joined_component_access.join(component_access);
                    joined_resource_access.join(resource_access);

                    systems.push(j);
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
