use super::{access::Access, param::SystemFunc, ECSError, ECS};

pub trait System {
    fn execute(&self, ecs: &ECS);
    fn component_access(&self) -> &Access;
    fn resource_access(&self) -> &Access;
    fn is_comp(&self, other_component_access: &Access, other_resource_access: &Access) -> bool {
        self.component_access().is_compatible(other_component_access) &&
        self.resource_access().is_compatible(other_resource_access)
    }
}

#[derive(Default)]
pub struct Schedule {
    systems: Vec<Box<dyn System + Send + Sync>>,
    parallel_exeuction_queue: Vec<Vec<usize>>,
}

impl Schedule {
    pub fn add_system<M, T: IntoSystem<M> + 'static>(&mut self, system: T) -> Result<(), ECSError> {
        let system = system.into_system();
        let component_access = system.component_access();
        if component_access.mutable_count as usize > component_access.mutable.len() {
            return Err(ECSError::MultipleMutRefs);
        }
        if component_access.immutable.intersection(&component_access.mutable).next().is_some() {
            return Err(ECSError::IncompatibleRefs);
        }
        self.systems.push(system);
        self.update_parallel_execution_queue();
        Ok(())
    }

    pub fn execute(&self, ecs: &ECS) {
        ecs.thread_pool.in_place_scope(|scope| {
            for pack in &self.parallel_exeuction_queue {
                for i in pack {
                    let system = &self.systems[*i];
                    scope.spawn(|_| {
                        system.execute(ecs);
                    });
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

unsafe impl Send for Schedule {}
unsafe impl Sync for Schedule {}

pub struct FunctionSystem<M, F: SystemFunc<M>> {
    component_access: Access,
    resource_access: Access,
    func: F,
    _a: std::marker::PhantomData<M>,
}

impl<M, F: SystemFunc<M>> System for FunctionSystem<M, F> {
    fn execute(&self, ecs: &ECS) {
        self.func.run(ecs);
    }

    fn component_access(&self) -> &Access {
        &self.component_access
    }

    fn resource_access(&self) -> &Access {
        &self.resource_access
    }
}

pub trait IntoSystem<M> {
    fn into_system(self) -> Box<dyn System + Send + Sync>;
}

impl<M: Send + Sync + 'static, T: Send + Sync + 'static> IntoSystem<M> for T where T: SystemFunc<M> + 'static {
    fn into_system(self) -> Box<dyn System + Send + Sync> {
        let mut component_access = Access::default();
        let mut resource_access = Access::default();
        Self::join_component_access(&mut component_access);
        Self::join_resource_access(&mut resource_access);
        Box::new(FunctionSystem {
            component_access,
            resource_access,
            func: self,
            _a: Default::default(),
        })
    }
}
