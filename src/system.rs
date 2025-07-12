use crate::{access::Access, component_manager::ComponentManager, ecs::ECSError, param::SystemFunc, resource::ResourceManager};

pub trait System {
    fn execute(&self, component_manager: &ComponentManager, resource_manager: &ResourceManager);
    fn component_access(&self) -> Access;
    fn resource_access(&self) -> Access;
    fn is_compatible_component_wise(&self, other: &dyn System) -> bool {
        let component_access = self.component_access();
        let other_component_access = other.component_access();
        
        let check_a = other_component_access.mutable & component_access.immutable;
        let check_b = component_access.mutable & other_component_access.immutable;
        let check_c = component_access.mutable & other_component_access.mutable;

        check_a.is_zero() && check_b.is_zero() && check_c.is_zero()
    }
    fn is_compatible_resource_wise(&self, other: &dyn System) -> bool {
        let resource_access = self.resource_access();
        let other_resource_access = other.resource_access();
        
        let check_a = other_resource_access.mutable & resource_access.immutable;
        let check_b = resource_access.mutable & other_resource_access.immutable;
        let check_c = resource_access.mutable & other_resource_access.mutable;

        check_a.is_zero() && check_b.is_zero() && check_c.is_zero()
    }
    fn is_compatible(&self, other: &dyn System) -> bool {
        self.is_compatible_component_wise(other) && self.is_compatible_resource_wise(other)
    }
    fn is_comp(&self, other_component_access: &Access, other_resource_access: &Access) -> bool {
        let component_access = self.component_access();
        let resource_access = self.resource_access();

        let check_a = other_component_access.mutable & component_access.immutable;
        let check_b = component_access.mutable & other_component_access.immutable;
        let check_c = component_access.mutable & other_component_access.mutable;

        let check_d = other_resource_access.mutable & resource_access.immutable;
        let check_e = resource_access.mutable & other_resource_access.immutable;
        let check_f = resource_access.mutable & other_resource_access.mutable;

        check_a.is_zero() && check_b.is_zero() && check_c.is_zero() && check_d.is_zero() && check_e.is_zero() && check_f.is_zero()
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
        if component_access.mutable_count > component_access.mutable.ones() {
            return Err(ECSError::MultipleMutRefs);
        }
        if !(component_access.immutable & component_access.mutable).is_zero() {
            return Err(ECSError::IncompatibleRefs);
        }
        self.systems.push(system);
        self.update_parallel_execution_queue();
        Ok(())
    }

    pub(crate) fn execute(&self, component_manager: &ComponentManager, resource_manager: &ResourceManager, thread_pool: &rayon::ThreadPool) {
        thread_pool.in_place_scope(|scope| {
            for pack in &self.parallel_exeuction_queue {
                for i in pack {
                    let system = &self.systems[*i];
                    scope.spawn(|_| {
                        system.execute(component_manager, resource_manager);
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
            let mut joined_component_access = system.component_access();
            let mut joined_resource_access = system.resource_access();

            for j in i+1..self.systems.len() {
                if added_systems[j] { continue; }
                let other_system = self.systems[j].as_ref();

                if other_system.is_comp(&joined_component_access, &joined_resource_access) {
                    added_systems[j] = true;
                    let component_access = other_system.component_access();
                    let resource_access = other_system.resource_access();
                    joined_component_access.immutable |= component_access.immutable;
                    joined_component_access.mutable |= component_access.mutable;
                    joined_resource_access.immutable |= resource_access.immutable;
                    joined_resource_access.mutable |= resource_access.mutable;
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

impl<M, F:SystemFunc<M>> System for FunctionSystem<M, F> {
    fn execute(&self, component_manager: &ComponentManager, resource_manager: &ResourceManager) {
        self.func.run(component_manager, resource_manager);
    }

    fn component_access(&self) -> Access {
        self.component_access
    }

    fn resource_access(&self) -> Access {
        self.resource_access
    }
}

pub trait IntoSystem<M> {
    fn into_system(self) -> Box<dyn System + Send + Sync>;
}

impl<M: Send + Sync + 'static, T: Send + Sync + 'static> IntoSystem<M> for T where T: SystemFunc<M> + 'static {
    fn into_system(self) -> Box<dyn System + Send + Sync> {
        Box::new(FunctionSystem {
            component_access: self.component_access(),
            resource_access: self.resource_access(),
            func: self,
            _a: Default::default(),
        })
    }
}
