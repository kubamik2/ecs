use std::any::TypeId;

use crate::{Commands, SystemHandle, World, access::Access, param::{SystemParam, SystemParamError}, system::SystemFunc, world::WorldPtr};

pub type ECSError = anyhow::Error;

pub mod handlers {
    use crate::error::ECSError;
    pub fn warn_error_handler(err: ECSError) {
        log::warn!("{}", err);
    }

    pub fn panic_error_handler(err: ECSError) {
        panic!("{}", err);
    }
}

impl<F, In> SystemFunc<(ECSError, In), ECSError, ()> for F where 
    F: Send + Sync + 'static,
    for<'a> &'a F:
        FnMut(ECSError, In) +
        FnMut(ECSError, In::Item<'a>),
    In: for<'a> SystemParam + 'static,
{
    type State = In::State;
    fn run<'a>(&self, world_ptr: WorldPtr<'a>, state: &'a mut Self::State, input: ECSError, system_meta: SystemHandle) {
        fn call<In>(mut f: impl FnMut(ECSError, In), s: ECSError, p: In) {
            f(s, p)
        }
        unsafe {
            let p = In::fetch(world_ptr, state, &system_meta);
            call(self, input, p)
        }
    }

    fn join_component_access(world: &mut World, component_access: &mut Access) -> Result<(), SystemParamError> {
        In::join_component_access(world, component_access)
    }

    fn join_resource_access(world: &mut World, resource_access: &mut Access) -> Result<(), SystemParamError> {
        In::join_resource_access(world, resource_access)
    }

    fn name(&self) -> &'static str {
        std::any::type_name::<F>()
    }

    fn init_state(world: &mut World, system_handle: SystemHandle) -> Result<Self::State, SystemParamError> {
        In::init_state(world, &system_handle)
    }

    fn trigger_access() -> Option<TypeId> {
        None
    }

    fn after<'state>(commands: &mut Commands<'state>, state: &'state mut Self::State) {
        In::after(commands, state);
    }
}

impl<F, Output> SystemFunc<ECSError, ECSError, Output> for F where 
    F: Send + Sync + 'static,
    for<'a> &'a F: FnMut(ECSError) -> Output
{
    type State = ();
    fn run<'a>(&self, _: WorldPtr<'a>, _: &'a mut Self::State, input: ECSError, _: SystemHandle) -> Output {
        fn call<Output>(mut f: impl FnMut(ECSError) -> Output, s: ECSError) -> Output {
            f(s)
        }
        call(self, input)
    }

    fn join_component_access(_: &mut World, _: &mut Access) -> Result<(), SystemParamError> { Ok(()) }

    fn join_resource_access(_: &mut World, _: &mut Access) -> Result<(), SystemParamError> { Ok(()) }

    fn name(&self) -> &'static str {
        std::any::type_name::<F>()
    }

    fn init_state(_: &mut World, _: SystemHandle) -> Result<Self::State, SystemParamError> { Ok(()) }

    fn trigger_access() -> Option<TypeId> {
        None
    }

    fn after(_: &mut Commands, _: &mut Self::State) {}
}

macro_rules! error_handler_func_impl {
    ($(($i:tt, $param:ident, $p:ident)),+) => {
        #[allow(unused_parens)]
        impl<'b, F, $($param),+, Output> SystemFunc<(ECSError, $($param),+), ECSError, Output> for F where
            F: Send + Sync + 'static,
            for<'a> &'a F: 
                FnMut(ECSError, $($param),+) -> Output +
                FnMut(ECSError, $($param::Item<'a>),+) -> Output,
            $($param: for<'a> SystemParam + 'static),+
        {
            type State = ($($param::State),+);
            fn run<'a>(&self, world_ptr: WorldPtr<'a>, state: &'a mut Self::State, input: ECSError, system_meta: SystemHandle) -> Output {
                #[allow(clippy::too_many_arguments)]
                fn call<'a, $($param),+, Output>(mut f: impl FnMut(ECSError, $($param),+) -> Output, s: ECSError, $($p:$param),+) -> Output {
                    f(s, $($p),+)
                }
                unsafe {
                    $(let $p = $param::fetch(world_ptr, &mut state.$i, &system_meta);)+
                    call(self, input, $($p),+)
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

variadics_please::all_tuples_enumerated!{error_handler_func_impl, 2, 31, In, p}

pub trait ErrorHandlerInput {}

impl ErrorHandlerInput for ECSError {}

macro_rules! error_handler_input_impl {
    ($($param:ident),+) => {
        #[allow(unused_parens)]
        impl<'a, $($param: SystemParam),+> ErrorHandlerInput for (ECSError, $($param),+) {}
    }
}

variadics_please::all_tuples!{error_handler_input_impl, 1, 31, In}
