use std::{any::{Any, TypeId}, collections::HashMap, hash::Hash, ptr::NonNull};

use crate::{access::Access, system::{IntoSystem, System, SystemId, SystemInput, SYSTEM_IDS}, world::{World, WorldId, WorldPtr}};

const PARALLEL_EXECUTION_THRESHOLD: usize = 4;

#[derive(Default)]
pub struct Schedule {
    linked_world: Option<WorldId>,
    system_records: Vec<SystemRecord>,
    parallel_execution_queue: Vec<ParallelBucket>,
    init_queue: Vec<Box<dyn System<Input = ()> + Send + Sync>>,
}

struct SystemRecord {
    system: Box<dyn System<Input = ()> + Send + Sync>,
    bucket_index: usize,
}

impl Schedule {
    pub fn add_system<ParamInput: SystemInput, S: IntoSystem<ParamInput, ()> + 'static>(&mut self, system: S) -> SystemId {
        let system: S::System = system.into_system();
        let id = system.id();

        self.init_queue.push(Box::new(system));
        id
    }

    pub(crate) fn add_boxed_system<S: crate::system::System<Input = ()> + Send + Sync + 'static>(&mut self, system: Box<S>) {
        self.init_queue.push(system);
    }

    pub fn init_system(&mut self, world: &mut World, mut system: Box<dyn System<Input = ()> + Send + Sync>) {
        system.init(world);

        // add system to parallel_exeuction_queue
        let maybe_bucket = self.parallel_execution_queue
            .iter_mut()
            .enumerate()
            .find(|(_, bucket)| {
                bucket.is_system_compatible(system.as_ref())
            });

        let bucket_index = match maybe_bucket {
            Some((index, bucket)) => {
                bucket.add_system(NonNull::from(system.as_ref()));
                index
            },
            None => {
                let mut bucket = ParallelBucket::default();
                bucket.add_system(NonNull::from(system.as_ref()));
                self.parallel_execution_queue.push(bucket);
                self.parallel_execution_queue.len()-1
            }
        };

        self.system_records.push(SystemRecord {
            bucket_index,
            system,
        });
    }

    pub(crate) fn execute(&mut self, mut world_ptr: WorldPtr<'_>) {
        for bucket in &self.parallel_execution_queue {
            if bucket.should_run_paralell {
                unsafe { world_ptr.as_world() }.thread_pool.in_place_scope(|scope| {
                    for mut system_ptr in bucket.systems.iter().copied() {
                        let system = unsafe { system_ptr.as_mut() };
                        scope.spawn(move |_| {
                            system.execute(world_ptr, ());
                        });
                    }
                });
            } else {
                for mut system_ptr in bucket.systems.iter().copied() {
                    let system = unsafe { system_ptr.as_mut() };
                    system.execute(world_ptr, ());
                }
            }
        }

        let world = unsafe { world_ptr.as_world_mut() };
        for i in 0..self.system_records.len() {
            let SystemRecord { system, bucket_index: _ } = &mut self.system_records[i];
            system.after(world.command_buffer());
        }

        let system_ids = SYSTEM_IDS.read().unwrap();
        let mut i = 0;
        while i < self.system_records.len() {
            let system_id = self.system_records[i].system.id();
            if !system_ids.is_alive(system_id.get()) {
                self.remove_system_at(i);
            } else {
                i += 1;
            }
        }
        drop(system_ids);
    }

    pub fn run(&mut self, world: &mut World) {
        let world_id = self.linked_world.unwrap_or(world.id());
        assert!(world.id() == world_id, "initialized schedule ran in a different world");
    
        while let Some(system) = self.init_queue.pop() {
            self.init_system(world, system);
        }

        self.execute(world.world_ptr_mut());
        world.process_command_buffer();
    }

    pub fn remove_system_at(&mut self, index: usize) {
        let SystemRecord { system, bucket_index } = &self.system_records[index];
        let id = system.id();
        let bucket_index = *bucket_index;

        // get the bucket and system index in bucket
        let bucket = &mut self.parallel_execution_queue[bucket_index];
        let Some(position) = bucket.systems
            .iter()
            .position(|p| unsafe { p.as_ref() }.id() == id)
            else { panic!("system not in bucket"); };
        
        // remove system from bucket
        bucket.systems.swap_remove(position);

        // recreate the bucket
        bucket.joined_component_access.clear();
        bucket.joined_resource_access.clear();
        for mut system_ptr in bucket.systems.iter().copied() {
            let system = unsafe { system_ptr.as_mut() };
            bucket.joined_component_access.join(system.component_access());
            bucket.joined_resource_access.join(system.resource_access());
        }

        // remove the system
        self.system_records.swap_remove(index);
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.system_records.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.system_records.is_empty()
    }
}

#[derive(Default)]
struct ParallelBucket {
    joined_component_access: Access,
    joined_resource_access: Access,
    systems: Vec<NonNull<dyn System<Input = ()> + Send + Sync>>,
    should_run_paralell: bool,
}

impl ParallelBucket {
    fn is_system_compatible(&self, system: &dyn System<Input = ()>) -> bool {
        self.joined_component_access.is_compatible(system.component_access()) &&
        self.joined_resource_access.is_compatible(system.resource_access())
    }

    fn add_system(&mut self, system: NonNull<dyn System<Input = ()> + Send + Sync>) {
        self.joined_component_access.join(unsafe { system.as_ref().component_access() });
        self.joined_resource_access.join(unsafe { system.as_ref().resource_access() });
        self.systems.push(system);
        self.should_run_paralell |= self.systems.len() >= PARALLEL_EXECUTION_THRESHOLD;
    }
}

pub trait ScheduleLabel: 'static + PartialEq + Eq + Hash {}

#[derive(Default)]
pub struct Schedules(HashMap<TypeId, Box<dyn Any>>);

impl Schedules {
    pub fn get_mut<L: ScheduleLabel>(&mut self, label: &L) -> Option<&mut Schedule> {
        let boxed_type_schedules = self.0.get_mut(&TypeId::of::<L>())?;
        let type_schedules = unsafe { boxed_type_schedules.downcast_mut_unchecked::<HashMap<L, Schedule>>() };
        type_schedules.get_mut(label)
    }

    pub fn insert<L: ScheduleLabel>(&mut self, label: L, schedule: Schedule) {
        let boxed_type_schedules = self.0
            .entry(TypeId::of::<L>())
            .or_insert(Box::new(HashMap::<L, Schedule>::default()));
        let type_schedules = unsafe { boxed_type_schedules.downcast_mut_unchecked::<HashMap<L, Schedule>>() };
        type_schedules.insert(label, schedule);
    }

    pub fn remove<L: ScheduleLabel>(&mut self, label: &L) -> Option<Schedule> {
        let boxed_type_schedules = self.0.get_mut(&TypeId::of::<L>())?;
        let type_schedules = unsafe { boxed_type_schedules.downcast_mut_unchecked::<HashMap<L, Schedule>>() };
        let res = type_schedules.remove(label);

        if type_schedules.is_empty() {
            self.0.remove(&TypeId::of::<L>());
        }

        res
    }

    pub fn get_or_default<L: ScheduleLabel>(&mut self, label: L) -> &mut Schedule {
        let boxed_type_schedules = self.0.entry(TypeId::of::<L>()).or_insert_with(|| Box::new(HashMap::<L, Schedule>::default()));
        let type_schedules = unsafe { boxed_type_schedules.downcast_mut_unchecked::<HashMap<L, Schedule>>() };
        type_schedules.entry(label).or_default()
    }
}
