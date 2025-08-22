use crate::{access::SignalAccess, entity::EntityBundle, param::SystemParam, Component, Entity};

use super::{access::Access, World};

pub trait System {
    fn name(&self) -> &'static str;
    fn execute(&mut self, world: &World);
    fn component_access(&self) -> &Access;
    fn resource_access(&self) -> &Access;
    fn signal_access(&self) -> &SignalAccess;
    fn init_state(&mut self, world: &mut World);
    fn is_comp(&self, other_component_access: &Access, other_resource_access: &Access) -> bool {
        self.component_access().is_compatible(other_component_access) &&
        self.resource_access().is_compatible(other_resource_access)
    }
    fn validate(&self) -> Result<(), SystemValidationError> {
        let component_access = self.component_access();
        let resource_access = self.resource_access();
        if component_access.mutable_count as usize > component_access.mutable.len() { return Err(SystemValidationError::MultipleComponentMutRefs);
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
    name: &'static str,
    state: Option<F::State>,
    component_access: Access,
    resource_access: Access,
    signal_access: SignalAccess,
    func: F,
    _a: std::marker::PhantomData<In>,
}

impl<In, F: SystemFunc<In>> System for FunctionSystem<In, F> {
    fn execute(&mut self, world: &World) {
        let name = self.name;
        let state = self.state.as_mut().unwrap_or_else(|| panic!("system '{}' has been executed without initialization", name));
        self.func.run(world, state);
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

    fn name(&self) -> &'static str {
        self.name
    }

    fn init_state(&mut self, world: &mut World) {
        self.state = Some(F::init_state(world));
    }
}

pub trait SystemFunc<In> {
    type State: Send + Sync;
    fn name(&self) -> &'static str;
    fn run(&self, world: &World, state: &mut Self::State);
    fn join_component_access(component_access: &mut Access);
    fn join_resource_access(resource_access: &mut Access);
    fn init_state(world: &mut World) -> Self::State;
}

impl<F> SystemFunc<()> for F where 
    F: Send + Sync + 'static,
    for<'a> &'a F: FnMut()
{
    type State = ();
    fn run(&self, _: &World, _: &mut Self::State) {
        fn call(mut f: impl FnMut()) {
            f()
        }
        call(self)
    }

    fn join_resource_access(_: &mut Access) {}

    fn join_component_access(_: &mut Access) {}

    fn name(&self) -> &'static str {
        std::any::type_name::<F>()
    }

    fn init_state(_: &mut World) -> Self::State {}
}

impl<F, In> SystemFunc<In> for F where 
    F: Send + Sync + 'static,
    for<'a> &'a F:
        FnMut(In) +
        FnMut(In::Item<'a>),
    In: for<'a> SystemParam + 'static,
{
    type State = In::State;
    fn run(&self, world: &World, state: &mut Self::State) {
        fn call<In>(mut f: impl FnMut(In), p: In) {
            f(p)
        }
        let p = In::fetch(world, state);
        call(self, p);
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
}

macro_rules! system_func_impl {
    ($(($i:tt, $param:ident, $p:ident)),+) => {
        #[allow(unused_parens)]
        impl<F, $($param),+> SystemFunc<($($param),+)> for F where
            F: Send + Sync + 'static,
            for<'a> &'a F: 
                FnMut($($param),+) +
                FnMut($($param::Item<'a>),+),
            $($param: for<'a> SystemParam<Item<'a> = $param> + 'static),+
        {
            type State = ($($param::State),+);
            fn run(&self, world: &World, state: &mut Self::State) {
                #[allow(clippy::too_many_arguments)]
                fn call<$($param),+>(mut f: impl FnMut($($param),+), $($p:$param),+) {
                    f($($p),+);
                }
                $(let $p = $param::fetch(world, &mut state.$i);)+
                call(self, $($p),+);
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
        }
    }
}

variadics_please::all_tuples_enumerated!{system_func_impl, 2, 32, In, p}

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
            name: self.name(),
            state: None,
            component_access,
            resource_access,
            signal_access: SignalAccess::default(),
            func: self,
            _a: Default::default(),
        })
    }
}

pub(crate) enum SystemCommand {
    Spawn(Box<dyn FnOnce(&mut World) + Send>),
    Remove(Entity),
    SetComponent(Box<dyn FnOnce(&mut World) + Send>),
    RemoveComponent(Box<dyn FnOnce(&mut World) + Send>),
}

#[derive(Clone)]
pub struct Commands(std::sync::mpsc::Sender<SystemCommand>);

impl Commands {
    pub fn spawn<B: EntityBundle + 'static + Send>(&self, bundle: B) {
        self.0.send(SystemCommand::Spawn(Box::new(move |world| { bundle.spawn(world); }))).expect("Commands::spawn Sender error")
    }

    pub fn remove(&self, entity: Entity) {
        self.0.send(SystemCommand::Remove(entity)).expect("Commands::remove Sender error")
    }

    pub fn set_component<C: Component>(&self, entity: Entity, component: C) {
        self.0.send(SystemCommand::SetComponent(Box::new(move |world| world.set_component(entity, component)))).expect("Commands::set_component Sender error")
    }

    pub fn remove_component<C: Component>(&self, entity: Entity) {
        self.0.send(SystemCommand::RemoveComponent(Box::new(move |world| world.remove_component::<C>(entity)))).expect("Commands::remove_component Sender error")
    }
}

impl SystemParam for Commands {
    type Item<'a> = Self;
    type State = Self;
    fn init_state(world: &mut World) -> Self::State {
        Self(world.system_command_sender.clone())
    }

    fn fetch<'a>(_: &'a World, state: &'a mut Self::State) -> Self::Item<'a> {
        state.clone()
    }
}

pub trait SystemInput {}

macro_rules! system_input_impl {
    ($($param:ident),+) => {
        #[allow(unused_parens)]
        impl<$($param: SystemParam),+> SystemInput for ($($param),+) {}
    }
}

impl SystemInput for () {}

variadics_please::all_tuples!{system_input_impl, 1, 32, In}
