use crate::{access::SignalAccess, entity_manager::EntityBundle, param::{SystemParam, ObserverParam}, Component, Entity};

use super::{access::Access, ECS};

pub trait System {
    fn execute(&self, ecs: &ECS);
    fn component_access(&self) -> &Access;
    fn resource_access(&self) -> &Access;
    fn signal_access(&self) -> &SignalAccess;
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

pub struct FunctionSystem<In, F: SystemFunc<In>> {
    component_access: Access,
    resource_access: Access,
    signal_access: SignalAccess,
    func: F,
    _a: std::marker::PhantomData<In>,
}

pub trait SystemFunc<In> {
    fn run(&self, ecs: &ECS);
    fn join_component_access(component_access: &mut Access);
    fn join_resource_access(resource_access: &mut Access);
}

impl<In, F: SystemFunc<In>> System for FunctionSystem<In, F> {
    fn execute(&self, ecs: &ECS) {
        self.func.run(ecs);
    }

    fn component_access(&self) -> &Access {
        &self.component_access
    }

    fn resource_access(&self) -> &Access {
        &self.resource_access
    }

    fn signal_access(&self) -> &SignalAccess {
        &self.signal_access
    }
}

impl<F> SystemFunc<()> for F where 
    F: Send + Sync + 'static,
    for<'a> &'a F: FnMut()
{
    fn run(&self, _: &ECS) {
        fn call(mut f: impl FnMut()) {
            f()
        }
        call(self)
    }

    fn join_resource_access(_: &mut Access) {}

    fn join_component_access(_: &mut Access) {}
}

macro_rules! system_func_impl {
    ($(($param:ident, $p:ident)),+) => {
        #[allow(unused_parens)]
        impl<F, $($param),+> SystemFunc<($($param),+)> for F where
            F: Send + Sync + 'static,
            for<'a> &'a F: FnMut($($param),+),
            $($param: for<'a> SystemParam<Item<'a> = $param> + 'static),+
        {
            fn run(&self, ecs: &ECS) {
                #[allow(clippy::too_many_arguments)]
                fn call<$($param),+>(mut f: impl FnMut($($param),+), $($p:$param),+) {
                    f($($p),+);
                }
                $(let $p = $param::create(ecs);)+
                if $($p.is_some())&&+ {
                    unsafe {call(self, $($p.unwrap_unchecked()),+)};
                }
            }
            
            fn join_component_access(component_access: &mut Access) {
                $($param::join_component_access(component_access);)+
            }

            fn join_resource_access(resource_access: &mut Access) {
                $($param::join_resource_access(resource_access);)+
            }
        }
    }
}

variadics_please::all_tuples!{system_func_impl, 1, 32, In, p}

pub trait IntoSystem<In> {
    fn into_system(self) -> Box<dyn System + Send + Sync>;
}

impl<In: Send + Sync + 'static, T: Send + Sync + 'static> IntoSystem<In> for T where T: SystemFunc<In> + 'static {
    fn into_system(self) -> Box<dyn System + Send + Sync> {
        let mut component_access = Access::default();
        let mut resource_access = Access::default();
        Self::join_component_access(&mut component_access);
        Self::join_resource_access(&mut resource_access);
        Box::new(FunctionSystem {
            component_access,
            resource_access,
            signal_access: SignalAccess::default(),
            func: self,
            _a: Default::default(),
        })
    }
}

pub(crate) enum SystemCommand {
    Spawn(Box<dyn FnOnce(&mut ECS) + Send>),
    Remove(Entity),
    SetComponent(Box<dyn FnOnce(&mut ECS) + Send>),
    RemoveComponent(Box<dyn FnOnce(&mut ECS) + Send>),
}

pub struct Commands(std::sync::mpsc::Sender<SystemCommand>);

impl Commands {
    pub fn spawn<B: EntityBundle + 'static + Send>(&self, bundle: B) {
        self.0.send(SystemCommand::Spawn(Box::new(move |ecs| { bundle.spawn(ecs); }))).expect("Commands::spawn Sender error")
    }

    pub fn remove(&self, entity: Entity) {
        self.0.send(SystemCommand::Remove(entity)).expect("Commands::remove Sender error")
    }

    pub fn set_component<C: Component>(&self, entity: Entity, component: C) {
        self.0.send(SystemCommand::SetComponent(Box::new(move |ecs| ecs.set_component(entity, component)))).expect("Commands::set_component Sender error")
    }

    pub fn remove_component<C: Component>(&self, entity: Entity) {
        self.0.send(SystemCommand::RemoveComponent(Box::new(move |ecs| ecs.remove_component::<C>(entity)))).expect("Commands::remove_component Sender error")
    }
}

impl SystemParam for Commands {
    type Item<'a> = Commands;
    fn create(ecs: &ECS) -> Option<Self::Item<'_>> {
        Some(Self(ecs.system_command_sender.clone()))
    }
}

pub trait SystemInput {}

macro_rules! system_input_impl {
    ($($param:ident),+) => {
        #[allow(unused_parens)]
        impl<$($param: SystemParam),+> SystemInput for ($($param),+) {}
    }
}

variadics_please::all_tuples!{system_input_impl, 1, 32, In}

pub trait ObserverInput {}

macro_rules! observer_input_impl {
    ($($param:ident),+) => {
        #[allow(unused_parens)]
        impl<$($param: ObserverParam),+> ObserverInput for ($($param),+) {}
    }
}

variadics_please::all_tuples!{observer_input_impl, 1, 32, In}
