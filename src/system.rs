use super::{access::Access, param::SystemFunc, ECS};

pub trait System {
    fn execute(&self, ecs: &ECS);
    fn component_access(&self) -> &Access;
    fn resource_access(&self) -> &Access;
    fn is_comp(&self, other_component_access: &Access, other_resource_access: &Access) -> bool {
        self.component_access().is_compatible(other_component_access) &&
        self.resource_access().is_compatible(other_resource_access)
    }
    fn validate(&self) -> Result<(), SystemValidationError> {
        let component_access = self.component_access();
        let resource_access = self.resource_access();
        if component_access.mutable_count as usize > component_access.mutable.len() {
            return Err(SystemValidationError::MultipleComponentMutRefs);
        }
        if component_access.immutable.intersection(&component_access.mutable).next().is_some() {
            return Err(SystemValidationError::IncompatibleComponentRefs);
        }
        if resource_access.mutable_count as usize > resource_access.mutable.len() {
            return Err(SystemValidationError::MultipleResourceMutRefs);
        }
        if resource_access.immutable.intersection(&resource_access.mutable).next().is_some() {
            return Err(SystemValidationError::IncompatibleResourceRefs);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SystemValidationError {
    MultipleComponentMutRefs,
    IncompatibleComponentRefs,
    MultipleResourceMutRefs,
    IncompatibleResourceRefs,
}

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
