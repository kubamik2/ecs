mod commands;
pub use commands::Commands;
use std::{any::TypeId, marker::PhantomData, sync::RwLock};
use crate::{entity::Entities, param::SystemParam, world::WorldPtr, Entity};

use super::{access::Access, World};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SystemId(Entity);

impl SystemId {
    #[inline]
    pub(crate) const fn id(&self) -> Entity {
        self.0
    }
}

impl SystemId {
    pub fn is_alive(&self) -> bool {
        SYSTEM_IDS.read().unwrap().is_alive(self.0)
    }
}

pub(crate) static SYSTEM_IDS: RwLock<Entities> = RwLock::new(Entities::new());

pub trait System {
    type Input;
    fn id(&self) -> SystemId;
    fn name(&self) -> &'static str;
    fn execute(&mut self, world_ptr: WorldPtr<'_>, input: Self::Input);
    fn component_access(&self) -> &Access;
    fn resource_access(&self) -> &Access;
    fn signal_access(&self) -> Option<&TypeId>;
    fn init(&mut self, world: &mut World);
    fn after(&mut self, world: &mut World);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SystemValidationError {
    MultipleComponentMutRefs,
    IncompatibleComponentRefs,
    MultipleResourceMutRefs,
    IncompatibleResourceRefs,
}

pub struct FunctionSystem<ParamIn, Input, F: SystemFunc<ParamIn, Input>> {
    id: SystemId,
    name: &'static str,
    state: Option<F::State>,
    component_access: Option<Access>,
    resource_access: Option<Access>,
    signal_access: Option<TypeId>,
    func: F,
    _a: std::marker::PhantomData<ParamIn>,
}

impl<Input, ParamIn, F: SystemFunc<ParamIn, Input>> FunctionSystem<ParamIn, Input, F> {
    fn validate(&self) -> Result<(), SystemValidationError> {
        let component_access = self.component_access();
        let resource_access = self.resource_access();
        if component_access.mutable_count > component_access.mutable.ones() {
            return Err(SystemValidationError::MultipleComponentMutRefs);
        }
        if !(component_access.immutable & component_access.mutable).is_zero() {
            return Err(SystemValidationError::IncompatibleComponentRefs);
        }
        if resource_access.mutable_count > resource_access.mutable.ones() {
            return Err(SystemValidationError::MultipleResourceMutRefs);
        }
        if !(resource_access.immutable & resource_access.mutable).is_zero() {
            return Err(SystemValidationError::IncompatibleResourceRefs);
        }
        Ok(())
    }
}

impl<Input, ParamIn, F: SystemFunc<ParamIn, Input>> System for FunctionSystem<ParamIn, Input, F> {
    type Input = Input;

    fn execute(&mut self, world_ptr: WorldPtr<'_>, input: Self::Input) {
        let name = self.name;
        let state = self.state.as_mut().unwrap_or_else(|| panic!("system '{}' has been executed without initialization", name));
        let system_meta = SystemHandle {
            id: self.id,
            _m: PhantomData,
        };
        self.func.run(world_ptr, state, input, &system_meta);
    }

    fn component_access(&self) -> &Access {
        let name = self.name;
        self.component_access.as_ref().unwrap_or_else(|| panic!("system '{}' has not been initialized", name))
    }

    fn resource_access(&self) -> &Access {
        let name = self.name;
        self.component_access.as_ref().unwrap_or_else(|| panic!("system '{}' has not been initialized", name))
    }

    fn signal_access(&self) -> Option<&TypeId> {
        self.signal_access.as_ref()
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn init(&mut self, world: &mut World) {
        self.state = Some(F::init_state(world));
        let mut component_access = Access::default();
        let mut resource_access = Access::default();
        F::join_component_access(world, &mut component_access);
        F::join_resource_access(world, &mut resource_access);
        self.component_access = Some(component_access);
        self.resource_access = Some(resource_access);
        self.validate().unwrap();
    }

    fn after(&mut self, world: &mut World) {
        let name = self.name;
        let state = self.state.as_mut().unwrap_or_else(|| panic!("system '{}' has been executed without initialization", name));
        let system_meta = SystemHandle {
            id: self.id,
            _m: PhantomData,
        };
        F::after(world, state, system_meta);   
    }

    #[inline]
    fn id(&self) -> SystemId {
        self.id
    }
}

pub trait SystemFunc<ParamIn, Input> {
    type State: Send + Sync; fn name(&self) -> &'static str;
    fn run<'a>(&'a self, world_ptr: WorldPtr<'a>, state: &'a mut Self::State, input: Input, system_meta: &'a SystemHandle<'a>);
    fn join_component_access(world: &mut World, component_access: &mut Access);
    fn join_resource_access(world: &mut World, resource_access: &mut Access);
    fn signal_access() -> Option<TypeId>;
    fn init_state(world: &mut World) -> Self::State;
    fn after<'state>(world: &mut World, state: &'state mut Self::State, system_meta: SystemHandle<'state>);
}

impl<F, Input> SystemFunc<(), Input> for F where 
    F: Send + Sync + 'static,
    for<'a> &'a F: FnMut()
{
    type State = ();
    fn run<'a>(&'a self, _: WorldPtr<'a>, _: &'a mut Self::State, _: Input, _: &'a SystemHandle<'a>) {
        fn call(mut f: impl FnMut()) {
            f()
        }
        call(self)
    }

    fn join_resource_access(_: &mut World, _: &mut Access) {}

    fn join_component_access(_: &mut World, _: &mut Access) {}

    fn signal_access() -> Option<TypeId> {
        None
    }

    fn name(&self) -> &'static str {
        std::any::type_name::<F>()
    }

    fn init_state(_: &mut World) -> Self::State {}
    fn after(_: &mut World, _: &mut Self::State, _: SystemHandle) {}
}

impl<F, ParamIn, Input> SystemFunc<ParamIn, Input> for F where 
    F: Send + Sync + 'static,
    for<'a> &'a F:
        FnMut(ParamIn) +
        FnMut(ParamIn::Item<'a>),
    ParamIn: for<'a> SystemParam + 'static,
{
    type State = ParamIn::State;
    fn run<'a>(&'a self, world_ptr: WorldPtr<'a>, state: &'a mut Self::State, _: Input, system_meta: &'a SystemHandle<'a>) {
        fn call<In>(mut f: impl FnMut(In), p: In) {
            f(p)
        }
        let p = unsafe { ParamIn::fetch(world_ptr, state, system_meta) };
        call(self, p);
    }

    fn join_component_access(world: &mut World, component_access: &mut Access) {
        ParamIn::join_component_access(world, component_access);
    }

    fn join_resource_access(world: &mut World, resource_access: &mut Access) {
        ParamIn::join_resource_access(world, resource_access);
    }

    fn name(&self) -> &'static str {
        std::any::type_name::<F>()
    }

    fn init_state(world: &mut World) -> Self::State {
        ParamIn::init_state(world)
    }

    fn signal_access() -> Option<TypeId> {
        None
    }

    fn after<'a>(world: &'a mut World, state: &'a mut Self::State, mut system_meta: SystemHandle<'a>) {
        ParamIn::after(world, state, &mut system_meta);
    }
}

macro_rules! system_func_impl {
    ($(($i:tt, $param:ident, $p:ident)),+) => {
        #[allow(unused_parens)]
        impl<F, $($param),+, Input> SystemFunc<($($param),+), Input> for F where
            F: Send + Sync + 'static,
            for<'a> &'a F: 
                FnMut($($param),+) +
                FnMut($($param::Item<'a>),+),
            $($param: for<'a> SystemParam),+
        {
            type State = ($($param::State),+);
            fn run<'a>(&'a self, world_ptr: WorldPtr<'a>, state: &'a mut Self::State, _: Input, system_meta: &'a SystemHandle<'a>) {
                #[allow(clippy::too_many_arguments)]
                fn call<$($param),+>(mut f: impl FnMut($($param),+), $($p:$param),+) {
                    f($($p),+);
                }
                unsafe {
                    $(let $p = $param::fetch(world_ptr, &mut state.$i, system_meta);)+
                    call(self, $($p),+);
                }
            }
            
            fn join_component_access(world: &mut World, component_access: &mut Access) {
                $($param::join_component_access(world, component_access);)+
            }

            fn join_resource_access(world: &mut World, resource_access: &mut Access) {
                $($param::join_resource_access(world, resource_access);)+
            }

            fn name(&self) -> &'static str {
                std::any::type_name::<F>()
            }

            fn init_state(world: &mut World) -> Self::State {
                ($($param::init_state(world)),+)
            }

            fn signal_access() -> Option<TypeId> {
                None
            }

            fn after<'state>(world: &mut World, state: &'state mut Self::State, mut system_meta: SystemHandle<'state>) {
                $($param::after(world, &mut state.$i, &mut system_meta);)+
            }
        }
    }
}

variadics_please::all_tuples_enumerated!{system_func_impl, 2, 32, In, p}

pub trait IntoSystem<ParamIn, Input> {
    type System: System<Input = Input> + Send + Sync + 'static;
    fn into_system(self) -> Self::System;
}

impl<ParamIn, F, Input> IntoSystem<ParamIn, Input> for F
where 
    F: SystemFunc<ParamIn, Input> + 'static + Send + Sync,
    ParamIn: Send + Sync + 'static,
    Input: 'static
{
    type System = FunctionSystem<ParamIn, Input, F>;
    fn into_system(self) -> Self::System {
        FunctionSystem {
            id: SystemId(SYSTEM_IDS.write().unwrap().spawn()),
            name: self.name(),
            state: None,
            component_access: None,
            resource_access: None,
            signal_access: F::signal_access(),
            func: self,
            _a: Default::default(),
        }
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

// prepared struct for manipulating system directly
pub struct SystemHandle<'a> {
    id: SystemId,
    _m: PhantomData<&'a u8>
}

impl SystemHandle<'_> {
    pub fn id(&self) -> SystemId {
        self.id
    }
}

impl SystemParam for &SystemHandle<'_> {
    type Item<'a> = &'a SystemHandle<'a>;
    type State = ();
    unsafe fn fetch<'a>(_: WorldPtr<'a>, _: &'a mut Self::State, system_meta: &'a SystemHandle) -> Self::Item<'a> {
        system_meta
    }
    fn init_state(_: &mut World) -> Self::State {}
}
