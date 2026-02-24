mod commands;
pub mod error;
pub use commands::Commands;
use std::{any::TypeId, error::Error, marker::PhantomData, ops::{Deref, DerefMut}, sync::{Arc, atomic::{AtomicBool, Ordering}}};
use crate::{access::AccessError, error::ECSError, param::{SystemParam, SystemParamError}, system::error::InternalSystemError, world::WorldPtr};

use super::{access::Access, World};

#[derive(Clone)]
pub struct SystemId(Arc<AtomicBool>);

impl SystemId {
    #[inline]
    pub fn is_alive(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    #[inline]
    pub(crate) fn new() -> Self {
        Self(Arc::new(AtomicBool::new(true)))
    }

    #[inline]
    pub fn mark_dead(&self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

impl PartialEq for SystemId {
    fn eq(&self, other: &Self) -> bool {
        let addr = self.0.as_ptr().addr();
        let other_addr = other.0.as_ptr().addr();
        addr == other_addr
    }
}

pub trait System {
    type Input;
    fn id(&self) -> &SystemId;
    fn name(&self) -> &'static str;
    fn execute(&mut self, world_ptr: WorldPtr<'_>, input: Self::Input);
    fn component_access(&self) -> &Access;
    fn resource_access(&self) -> &Access;
    fn trigger_access(&self) -> Option<&TypeId>;
    fn init(&mut self, world: &mut World) -> Result<(), InternalSystemError>;
    fn is_init(&self) -> bool;
    fn after<'a>(&'a mut self, commands: Commands<'a>);
}

pub struct FunctionSystem<ParamIn, Input, Output, F: SystemFunc<ParamIn, Input, Output>> {
    id: SystemId,
    name: &'static str,
    state: Option<F::State>,
    component_access: Option<Access>,
    resource_access: Option<Access>,
    trigger_access: Option<TypeId>,
    is_init: bool,
    func: F,
    last_error: Option<ECSError>,
    _a: std::marker::PhantomData<ParamIn>,
}

impl<Input, ParamIn, Output, F: SystemFunc<ParamIn, Input, Output>> FunctionSystem<ParamIn, Input, Output, F> {
    fn validate(&self) -> Result<(), AccessError> {
        self.component_access.as_ref().unwrap_or_else(|| panic!("system '{}' has not been initialized", self.name)).validate()?;
        self.resource_access.as_ref().unwrap_or_else(|| panic!("system '{}' has not been initialized", self.name)).validate()?;
        Ok(())
    }
}

impl<Input, ParamIn, F: SystemFunc<ParamIn, Input, ()>> System for FunctionSystem<ParamIn, Input, (), F> {
    type Input = Input;

    fn execute(&mut self, world_ptr: WorldPtr<'_>, input: Self::Input) {
        let name = self.name;
        let state = self.state.as_mut().unwrap_or_else(|| panic!("system '{}' has been executed without initialization", name));
        let system_meta = SystemHandle {
            id: &self.id,
            name,
            _m: PhantomData,
        };
        self.func.run(world_ptr, state, input, system_meta);
    }

    #[inline]
    fn component_access(&self) -> &Access {
        let name = self.name;
        self.component_access.as_ref().unwrap_or_else(|| panic!("system '{}' has not been initialized", name))
    }

    #[inline]
    fn resource_access(&self) -> &Access {
        let name = self.name;
        self.resource_access.as_ref().unwrap_or_else(|| panic!("system '{}' has not been initialized", name))
    }

    #[inline]
    fn trigger_access(&self) -> Option<&TypeId> {
        self.trigger_access.as_ref()
    }

    #[inline]
    fn name(&self) -> &'static str {
        self.name
    }

    #[inline]
    fn init(&mut self, world: &mut World) -> Result<(), InternalSystemError> {
        let system_handle = SystemHandle {
            name: self.name,
            id: &self.id,
            _m: PhantomData,
        };
        let mut component_access = Access::default();
        let mut resource_access = Access::default();
        F::join_component_access(world, &mut component_access).map_err(|err| InternalSystemError::param(self.name, self.id.clone(), err))?;
        F::join_resource_access(world, &mut resource_access).map_err(|err| InternalSystemError::param(self.name, self.id.clone(), err))?;
        self.component_access = Some(component_access);
        self.resource_access = Some(resource_access);
        self.validate().map_err(|err| InternalSystemError::access(self.name, self.id.clone(), err))?;
        self.state = Some(F::init_state(world, system_handle).map_err(|err| InternalSystemError::param(self.name, self.id.clone(), err))?);
        self.is_init = true;
        Ok(())
    }

    #[inline]
    fn after<'a>(&'a mut self, mut commands: Commands<'a>) {
        let name = self.name;
        let state = self.state.as_mut().unwrap_or_else(|| panic!("system '{}' has been executed without initialization", name));
        F::after(&mut commands, state);   
    }

    #[inline]
    fn id(&self) -> &SystemId {
        &self.id
    }

    fn is_init(&self) -> bool {
        self.is_init
    }
}

impl<Input, ParamIn, E: Error + Send + Sync + 'static, F: SystemFunc<ParamIn, Input, Result<(), E>>> System for FunctionSystem<ParamIn, Input, Result<(), E>, F> {
    type Input = Input;

    fn execute(&mut self, world_ptr: WorldPtr<'_>, input: Self::Input) {
        let name = self.name;
        let state = self.state.as_mut().unwrap_or_else(|| panic!("system '{}' has been executed without initialization", name));
        let system_meta = SystemHandle {
            id: &self.id,
            name,
            _m: PhantomData,
        };
        self.last_error = self.func.run(world_ptr, state, input, system_meta).err().map(|err| ECSError::from(err));
    }

    #[inline]
    fn component_access(&self) -> &Access {
        let name = self.name;
        self.component_access.as_ref().unwrap_or_else(|| panic!("system '{}' has not been initialized", name))
    }

    #[inline]
    fn resource_access(&self) -> &Access {
        let name = self.name;
        self.resource_access.as_ref().unwrap_or_else(|| panic!("system '{}' has not been initialized", name))
    }

    #[inline]
    fn trigger_access(&self) -> Option<&TypeId> {
        self.trigger_access.as_ref()
    }

    #[inline]
    fn name(&self) -> &'static str {
        self.name
    }

    #[inline]
    fn init(&mut self, world: &mut World) -> Result<(), InternalSystemError> {
        let system_handle = SystemHandle {
            name: self.name,
            id: &self.id,
            _m: PhantomData,
        };
        let mut component_access = Access::default();
        let mut resource_access = Access::default();
        F::join_component_access(world, &mut component_access).map_err(|err| InternalSystemError::param(self.name, self.id.clone(), err))?;
        F::join_resource_access(world, &mut resource_access).map_err(|err| InternalSystemError::param(self.name, self.id.clone(), err))?;
        self.component_access = Some(component_access);
        self.resource_access = Some(resource_access);
        self.validate().map_err(|err| InternalSystemError::access(self.name, self.id.clone(), err))?;
        self.state = Some(F::init_state(world, system_handle).map_err(|err| InternalSystemError::param(self.name, self.id.clone(), err))?);
        self.is_init = true;
        Ok(())
    }

    #[inline]
    fn after<'a>(&'a mut self, mut commands: Commands<'a>) {
        let name = self.name;
        let state = self.state.as_mut().unwrap_or_else(|| panic!("system '{}' has been executed without initialization", name));
        F::after(&mut commands, state);   
        if let Some(error) = self.last_error.take() {
            commands.handle_error(error);
        }
    }

    #[inline]
    fn id(&self) -> &SystemId {
        &self.id
    }

    fn is_init(&self) -> bool {
        self.is_init
    }
}

pub trait SystemFunc<ParamIn, Input, Output> {
    type State: Send + Sync; fn name(&self) -> &'static str;
    fn run<'a>(&'a self, world_ptr: WorldPtr<'a>, state: &'a mut Self::State, input: Input, system_meta: SystemHandle<'a>) -> Output;
    fn join_component_access(world: &mut World, component_access: &mut Access) -> Result<(), SystemParamError>;
    fn join_resource_access(world: &mut World, resource_access: &mut Access) -> Result<(), SystemParamError>;
    fn trigger_access() -> Option<TypeId>;
    fn init_state(world: &mut World, system_handle: SystemHandle) -> Result<Self::State, SystemParamError>;
    fn after<'state>(commands: &mut Commands<'state>, state: &'state mut Self::State);
}

impl<F, Output> SystemFunc<(), (), Output> for F where 
    F: Send + Sync + 'static,
    for<'a> &'a F: FnMut() -> Output
{
    type State = ();
    #[inline]
    fn run<'a>(&'a self, _: WorldPtr<'a>, _: &'a mut Self::State, _: (), _: SystemHandle<'a>) -> Output {
        fn call<Output>(mut f: impl FnMut() -> Output) -> Output {
            f()
        }
        call(self)
    }

    #[inline]
    fn join_resource_access(_: &mut World, _: &mut Access) -> Result<(), SystemParamError> { Ok(()) }

    #[inline]
    fn join_component_access(_: &mut World, _: &mut Access) -> Result<(), SystemParamError> { Ok(()) }

    #[inline]
    fn trigger_access() -> Option<TypeId> {
        None
    }

    #[inline]
    fn name(&self) -> &'static str {
        std::any::type_name::<F>()
    }

    #[inline]
    fn init_state(_: &mut World, _: SystemHandle) -> Result<Self::State, SystemParamError> { Ok(()) }

    #[inline]
    fn after(_: &mut Commands, _: &mut Self::State) {}
}

impl<F, ParamIn, Output> SystemFunc<ParamIn, (), Output> for F where 
    F: Send + Sync + 'static,
    for<'a> &'a F:
        FnMut(ParamIn) -> Output +
        FnMut(ParamIn::Item<'a>) -> Output,
    ParamIn: for<'a> SystemParam + 'static,
{
    type State = ParamIn::State;
    #[inline]
    fn run<'a>(&'a self, world_ptr: WorldPtr<'a>, state: &'a mut Self::State, _: (), system_meta: SystemHandle<'a>) -> Output {
        fn call<In, Output>(mut f: impl FnMut(In) -> Output, p: In) -> Output {
            f(p)
        }
        let p = unsafe { ParamIn::fetch(world_ptr, state, &system_meta) };
        call(self, p)
    }

    #[inline]
    fn join_component_access(world: &mut World, component_access: &mut Access) -> Result<(), SystemParamError> {
        ParamIn::join_component_access(world, component_access)
    }

    #[inline]
    fn join_resource_access(world: &mut World, resource_access: &mut Access) -> Result<(), SystemParamError> {
        ParamIn::join_resource_access(world, resource_access)
    }

    #[inline]
    fn name(&self) -> &'static str {
        std::any::type_name::<F>()
    }

    #[inline]
    fn init_state(world: &mut World, system_handle: SystemHandle) -> Result<Self::State, SystemParamError> {
        ParamIn::init_state(world, &system_handle)
    }

    #[inline]
    fn trigger_access() -> Option<TypeId> {
        None
    }

    #[inline]
    fn after<'a>(commands: &mut Commands<'a>, state: &'a mut Self::State) {
        ParamIn::after(commands, state);
    }
}

macro_rules! system_func_impl {
    ($(($i:tt, $param:ident, $p:ident)),+) => {
        #[allow(unused_parens)]
        impl<F, $($param),+, Output> SystemFunc<($($param),+), (), Output> for F where
            F: Send + Sync + 'static,
            for<'a> &'a F: 
                FnMut($($param),+) -> Output +
                FnMut($($param::Item<'a>),+) -> Output,
            $($param: for<'a> SystemParam),+
        {
            type State = ($($param::State),+);
            fn run<'a>(&'a self, world_ptr: WorldPtr<'a>, state: &'a mut Self::State, _: (), system_meta: SystemHandle<'a>) -> Output {
                #[allow(clippy::too_many_arguments)]
                fn call<$($param),+, Output>(mut f: impl FnMut($($param),+) -> Output, $($p:$param),+) -> Output {
                    f($($p),+)
                }
                unsafe {
                    $(let $p = $param::fetch(world_ptr, &mut state.$i, &system_meta);)+
                    call(self, $($p),+)
                }
            }
            
            fn join_component_access(world: &mut World, component_access: &mut Access) -> Result<(), SystemParamError> {
                $($param::join_component_access(world, component_access)?;)+
                Ok(())
            }

            fn join_resource_access(world: &mut World, resource_access: &mut Access) -> Result<(), SystemParamError> {
                $($param::join_resource_access(world, resource_access)?;)+
                Ok(())
            }

            fn name(&self) -> &'static str {
                std::any::type_name::<F>()
            }

            fn init_state(world: &mut World, system_handle: SystemHandle) -> Result<Self::State, SystemParamError> {
                Ok(($($param::init_state(world, &system_handle)?),+))
            }

            fn trigger_access() -> Option<TypeId> {
                None
            }

            fn after<'state>(commands: &mut Commands<'state>, state: &'state mut Self::State) {
                $($param::after(commands, &mut state.$i);)+
            }
        }
    }
}

variadics_please::all_tuples_enumerated!{system_func_impl, 2, 32, In, p}

pub trait IntoSystem<ParamIn, Input, Output> {
    type System: System<Input = Input> + Send + Sync + 'static;
    fn into_system(self) -> Self::System;
    fn into_system_with_id(self, id: SystemId) -> Self::System;
}

impl<ParamIn, F, Input> IntoSystem<ParamIn, Input, ()> for F
where 
    F: SystemFunc<ParamIn, Input, ()> + 'static + Send + Sync,
    ParamIn: Send + Sync + 'static,
    Input: 'static
{
    type System = FunctionSystem<ParamIn, Input, (), F>;
    fn into_system(self) -> Self::System {
        FunctionSystem {
            id: SystemId::new(),
            name: self.name(),
            state: None,
            component_access: None,
            resource_access: None,
            trigger_access: F::trigger_access(),
            is_init: false,
            func: self,
            last_error: None,
            _a: Default::default(),
        }
    }

    fn into_system_with_id(self, id: SystemId) -> Self::System {
        FunctionSystem {
            id,
            name: self.name(),
            state: None,
            component_access: None,
            resource_access: None,
            trigger_access: F::trigger_access(),
            is_init: false,
            func: self,
            last_error: None,
            _a: Default::default(),
        }
    }
}

impl<ParamIn, F, Input, E: Error + Send + Sync + 'static> IntoSystem<ParamIn, Input, Result<(), E>> for F
where 
    F: SystemFunc<ParamIn, Input, Result<(), E>> + 'static + Send + Sync,
    ParamIn: Send + Sync + 'static,
    Input: 'static
{
    type System = FunctionSystem<ParamIn, Input, Result<(), E>, F>;
    fn into_system(self) -> Self::System {
        FunctionSystem {
            id: SystemId::new(),
            name: self.name(),
            state: None,
            component_access: None,
            resource_access: None,
            trigger_access: F::trigger_access(),
            is_init: false,
            func: self,
            last_error: None,
            _a: Default::default(),
        }
    }

    fn into_system_with_id(self, id: SystemId) -> Self::System {
        FunctionSystem {
            id,
            name: self.name(),
            state: None,
            component_access: None,
            resource_access: None,
            trigger_access: F::trigger_access(),
            is_init: false,
            func: self,
            last_error: None,
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

pub trait SystemOutput {}

impl SystemOutput for () {}
impl<E: Error> SystemOutput for Result<(), E> {}

// prepared struct for manipulating system directly
pub struct SystemHandle<'a> {
    id: &'a SystemId,
    name: &'static str,
    _m: PhantomData<&'a u8>
}

impl SystemHandle<'_> {
    #[inline]
    pub const fn id(&self) -> &SystemId {
        self.id
    }

    #[inline]
    pub const fn name(&self) -> &'static str {
        self.name
    }
}

unsafe impl SystemParam for &SystemHandle<'_> {
    type Item<'a> = &'a SystemHandle<'a>;
    type State = ();
    unsafe fn fetch<'a>(_: WorldPtr<'a>, _: &'a mut Self::State, system_meta: &'a SystemHandle) -> Self::Item<'a> {
        system_meta
    }
    fn init_state(_: &mut World, _: &SystemHandle) -> Result<Self::State, SystemParamError> { Ok(()) }
}


pub struct Local<'a, T: Default + Send + Sync + 'static>(&'a mut T);

impl<T: Default + Send + Sync + 'static> Deref for Local<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<T: Default + Send + Sync + 'static> DerefMut for Local<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

unsafe impl<T: Default + Send + Sync + 'static> SystemParam for Local<'_, T> {
    type Item<'a> = Local<'a, T>;
    type State = T;

    #[inline]
    fn init_state(_: &mut World, _: &SystemHandle) -> Result<Self::State, SystemParamError> {
        Ok(T::default())
    }

    #[inline]
    unsafe fn fetch<'a>(_: WorldPtr<'a>, state: &'a mut Self::State, _: &'a SystemHandle<'a>) -> Self::Item<'a> {
        Local(state)
    }
}
