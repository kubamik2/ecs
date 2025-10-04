mod commands;
pub use commands::Commands;
use std::{any::TypeId, marker::PhantomData, ops::{Deref, DerefMut}, sync::{Arc, atomic::{AtomicBool, Ordering}}};
use crate::{param::SystemParam, world::WorldPtr};

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
    fn signal_access(&self) -> Option<&TypeId>;
    fn init(&mut self, world: &mut World);
    fn is_init(&self) -> bool;
    fn after(&mut self, commands: Commands);
}

pub struct FunctionSystem<ParamIn, Input, F: SystemFunc<ParamIn, Input>> {
    id: SystemId,
    name: &'static str,
    state: Option<F::State>,
    component_access: Option<Access>,
    resource_access: Option<Access>,
    signal_access: Option<TypeId>,
    is_init: bool,
    func: F,
    _a: std::marker::PhantomData<ParamIn>,
}

impl<Input, ParamIn, F: SystemFunc<ParamIn, Input>> FunctionSystem<ParamIn, Input, F> {
    fn validate(&self) {
        let component_access = self.component_access();
        let resource_access = self.resource_access();
        assert!(component_access.mutable_count == component_access.mutable.ones(), "'{}' system: duplicate mutable component references", self.name);
        assert!((component_access.immutable & component_access.mutable).is_zero(), "'{}' system: incompatible component references", self.name);
        assert!(resource_access.mutable_count == resource_access.mutable.ones(), "'{}' system: duplicate mutable resource references", self.name);
        assert!((resource_access.immutable & resource_access.mutable).is_zero(), "'{}' system: incompatible resource references", self.name);
    }
}

impl<Input, ParamIn, F: SystemFunc<ParamIn, Input>> System for FunctionSystem<ParamIn, Input, F> {
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
        self.component_access.as_ref().unwrap_or_else(|| panic!("system '{}' has not been initialized", name))
    }

    #[inline]
    fn signal_access(&self) -> Option<&TypeId> {
        self.signal_access.as_ref()
    }

    #[inline]
    fn name(&self) -> &'static str {
        self.name
    }

    #[inline]
    fn init(&mut self, world: &mut World) {
        let system_handle = SystemHandle {
            name: self.name,
            id: &self.id,
            _m: PhantomData,
        };
        self.state = Some(F::init_state(world, system_handle));
        let mut component_access = Access::default();
        let mut resource_access = Access::default();
        F::join_component_access(world, &mut component_access);
        F::join_resource_access(world, &mut resource_access);
        self.component_access = Some(component_access);
        self.resource_access = Some(resource_access);
        self.validate();
        self.is_init = true;
    }

    #[inline]
    fn after(&mut self, commands: Commands) {
        let name = self.name;
        let state = self.state.as_mut().unwrap_or_else(|| panic!("system '{}' has been executed without initialization", name));
        F::after(commands, state);   
    }

    #[inline]
    fn id(&self) -> &SystemId {
        &self.id
    }

    fn is_init(&self) -> bool {
        self.is_init
    }
}

pub trait SystemFunc<ParamIn, Input> {
    type State: Send + Sync; fn name(&self) -> &'static str;
    fn run<'a>(&'a self, world_ptr: WorldPtr<'a>, state: &'a mut Self::State, input: Input, system_meta: SystemHandle<'a>);
    fn join_component_access(world: &mut World, component_access: &mut Access);
    fn join_resource_access(world: &mut World, resource_access: &mut Access);
    fn signal_access() -> Option<TypeId>;
    fn init_state(world: &mut World, system_handle: SystemHandle) -> Self::State;
    fn after<'state>(commands: Commands<'state>, state: &'state mut Self::State);
}

impl<F, Input> SystemFunc<(), Input> for F where 
    F: Send + Sync + 'static,
    for<'a> &'a F: FnMut()
{
    type State = ();
    #[inline]
    fn run<'a>(&'a self, _: WorldPtr<'a>, _: &'a mut Self::State, _: Input, _: SystemHandle<'a>) {
        fn call(mut f: impl FnMut()) {
            f()
        }
        call(self)
    }

    #[inline]
    fn join_resource_access(_: &mut World, _: &mut Access) {}

    #[inline]
    fn join_component_access(_: &mut World, _: &mut Access) {}

    #[inline]
    fn signal_access() -> Option<TypeId> {
        None
    }

    #[inline]
    fn name(&self) -> &'static str {
        std::any::type_name::<F>()
    }

    #[inline]
    fn init_state(_: &mut World, _: SystemHandle) -> Self::State {}

    #[inline]
    fn after(_: Commands, _: &mut Self::State) {}
}

impl<F, ParamIn, Input> SystemFunc<ParamIn, Input> for F where 
    F: Send + Sync + 'static,
    for<'a> &'a F:
        FnMut(ParamIn) +
        FnMut(ParamIn::Item<'a>),
    ParamIn: for<'a> SystemParam + 'static,
{
    type State = ParamIn::State;
    #[inline]
    fn run<'a>(&'a self, world_ptr: WorldPtr<'a>, state: &'a mut Self::State, _: Input, system_meta: SystemHandle<'a>) {
        fn call<In>(mut f: impl FnMut(In), p: In) {
            f(p)
        }
        let p = unsafe { ParamIn::fetch(world_ptr, state, &system_meta) };
        call(self, p);
    }

    #[inline]
    fn join_component_access(world: &mut World, component_access: &mut Access) {
        ParamIn::join_component_access(world, component_access);
    }

    #[inline]
    fn join_resource_access(world: &mut World, resource_access: &mut Access) {
        ParamIn::join_resource_access(world, resource_access);
    }

    #[inline]
    fn name(&self) -> &'static str {
        std::any::type_name::<F>()
    }

    #[inline]
    fn init_state(world: &mut World, system_handle: SystemHandle) -> Self::State {
        ParamIn::init_state(world, &system_handle)
    }

    #[inline]
    fn signal_access() -> Option<TypeId> {
        None
    }

    #[inline]
    fn after<'a>(mut commands: Commands<'a>, state: &'a mut Self::State) {
        ParamIn::after(&mut commands, state);
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
            fn run<'a>(&'a self, world_ptr: WorldPtr<'a>, state: &'a mut Self::State, _: Input, system_meta: SystemHandle<'a>) {
                #[allow(clippy::too_many_arguments)]
                fn call<$($param),+>(mut f: impl FnMut($($param),+), $($p:$param),+) {
                    f($($p),+);
                }
                unsafe {
                    $(let $p = $param::fetch(world_ptr, &mut state.$i, &system_meta);)+
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

            fn init_state(world: &mut World, system_handle: SystemHandle) -> Self::State {
                ($($param::init_state(world, &system_handle)),+)
            }

            fn signal_access() -> Option<TypeId> {
                None
            }

            fn after<'state>(mut commands: Commands<'state>, state: &'state mut Self::State) {
                $($param::after(&mut commands, &mut state.$i);)+
            }
        }
    }
}

variadics_please::all_tuples_enumerated!{system_func_impl, 2, 32, In, p}

pub trait IntoSystem<ParamIn, Input> {
    type System: System<Input = Input> + Send + Sync + 'static;
    fn into_system(self) -> Self::System;
    fn into_system_with_id(self, id: SystemId) -> Self::System;
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
            id: SystemId::new(),
            name: self.name(),
            state: None,
            component_access: None,
            resource_access: None,
            signal_access: F::signal_access(),
            is_init: false,
            func: self,
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
            signal_access: F::signal_access(),
            is_init: false,
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

impl SystemParam for &SystemHandle<'_> {
    type Item<'a> = &'a SystemHandle<'a>;
    type State = ();
    unsafe fn fetch<'a>(_: WorldPtr<'a>, _: &'a mut Self::State, system_meta: &'a SystemHandle) -> Self::Item<'a> {
        system_meta
    }
    fn init_state(_: &mut World, _: &SystemHandle) -> Self::State {}
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

impl<T: Default + Send + Sync + 'static> SystemParam for Local<'_, T> {
    type Item<'a> = Local<'a, T>;
    type State = T;

    #[inline]
    fn init_state(_: &mut World, _: &SystemHandle) -> Self::State {
        T::default()
    }

    #[inline]
    unsafe fn fetch<'a>(_: WorldPtr<'a>, state: &'a mut Self::State, _: &'a SystemHandle<'a>) -> Self::Item<'a> {
        Local(state)
    }
}
