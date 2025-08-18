use crate::{system::{IntoSystem, System, SystemValidationError}, ECS};

#[derive(Default)]
pub struct Schedule {
    systems: Vec<Box<dyn System + Send + Sync>>,
    parallel_exeuction_queue: Vec<Vec<usize>>,
}

impl Schedule {
    pub fn add_system<M, T: IntoSystem<M> + 'static>(&mut self, system: T) -> Result<(), SystemValidationError> {
        let system = system.into_system();
        system.validate()?;
        self.systems.push(system);
        self.update_parallel_execution_queue();
        Ok(())
    }

    pub fn execute(&self, ecs: &ECS) {
        ecs.thread_pool.in_place_scope(|scope| {
            for pack in &self.parallel_exeuction_queue {
                if pack.len() > 4 {
                    for i in pack {
                        let system = &self.systems[*i];
                        scope.spawn(|_| {
                            system.execute(ecs);
                        });
                    }
                } else {
                    for i in pack {
                        let system = &self.systems[*i];
                        system.execute(ecs);
                    }
                }
            }
        });
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

pub trait ScheduleLabel: 'static {
    fn enumerator(&self) -> usize;
}
