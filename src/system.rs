use std::any::TypeId;

use crate::{entity::EntityBundle, param::SystemParam, world::WorldPtr, Component, Entity};

use super::{access::Access, World};

pub trait System {
    type Input;
    fn name(&self) -> &'static str;
    fn execute(&mut self, world_ptr: WorldPtr<'_>, input: Self::Input);
    fn component_access(&self) -> &Access;
    fn resource_access(&self) -> &Access;
    fn signal_access(&self) -> Option<&TypeId>;
    fn init_state(&mut self, world: &mut World);
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
    name: &'static str,
    state: Option<F::State>,
    component_access: Access,
    resource_access: Access,
    signal_access: Option<TypeId>,
    func: F,
    _a: std::marker::PhantomData<ParamIn>,
}

impl<Input, ParamIn, F: SystemFunc<ParamIn, Input>> System for FunctionSystem<ParamIn, Input, F> {
    type Input = Input;
    fn execute(&mut self, world_ptr: WorldPtr<'_>, input: Self::Input) {
        let name = self.name;
        let state = self.state.as_mut().unwrap_or_else(|| panic!("system '{}' has been executed without initialization", name));
        self.func.run(world_ptr, state, input);
    }

    fn component_access(&self) -> &Access {
        &self.component_access
    }

    fn resource_access(&self) -> &Access {
        &self.resource_access
    }

    fn signal_access(&self) -> Option<&TypeId> {
        self.signal_access.as_ref()
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn init_state(&mut self, world: &mut World) {
        self.state = Some(F::init_state(world));
    }

    fn after(&mut self, world: &mut World) {
        let name = self.name;
        let state = self.state.as_mut().unwrap_or_else(|| panic!("system '{}' has been executed without initialization", name));
        F::after(world, state);   
    }
}

pub trait SystemFunc<ParamIn, Input> {
    type State: Send + Sync;
    fn name(&self) -> &'static str;
    fn run<'a>(&self, world_ptr: WorldPtr<'a>, state: &'a mut Self::State, input: Input);
    fn join_component_access(component_access: &mut Access);
    fn join_resource_access(resource_access: &mut Access);
    fn signal_access() -> Option<TypeId>;
    fn init_state(world: &mut World) -> Self::State;
    fn after(world: &mut World, state: &mut Self::State);
}

impl<F, Input> SystemFunc<(), Input> for F where 
    F: Send + Sync + 'static,
    for<'a> &'a F: FnMut()
{
    type State = ();
    fn run<'a>(&self, _: WorldPtr<'a>, _: &'a mut Self::State, _: Input) {
        fn call(mut f: impl FnMut()) {
            f()
        }
        call(self)
    }

    fn join_resource_access(_: &mut Access) {}

    fn join_component_access(_: &mut Access) {}

    fn signal_access() -> Option<TypeId> {
        None
    }

    fn name(&self) -> &'static str {
        std::any::type_name::<F>()
    }

    fn init_state(_: &mut World) -> Self::State {}
    fn after(_: &mut World, _: &mut Self::State) {}
}

impl<F, ParamIn, Input> SystemFunc<ParamIn, Input> for F where 
    F: Send + Sync + 'static,
    for<'a> &'a F:
        FnMut(ParamIn) +
        FnMut(ParamIn::Item<'a>),
    ParamIn: for<'a> SystemParam + 'static,
{
    type State = ParamIn::State;
    fn run<'a>(&self, world_ptr: WorldPtr<'a>, state: &'a mut Self::State, _: Input) {
        fn call<In>(mut f: impl FnMut(In), p: In) {
            f(p)
        }
        let p = unsafe { ParamIn::fetch(world_ptr, state) };
        call(self, p);
    }

    fn join_component_access(component_access: &mut Access) {
        ParamIn::join_component_access(component_access);
    }

    fn join_resource_access(resource_access: &mut Access) {
        ParamIn::join_resource_access(resource_access);
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

    fn after(world: &mut World, state: &mut Self::State) {
        ParamIn::after(world, state);
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
            fn run<'a>(&self, world_ptr: WorldPtr<'a>, state: &'a mut Self::State, _: Input) {
                #[allow(clippy::too_many_arguments)]
                fn call<$($param),+>(mut f: impl FnMut($($param),+), $($p:$param),+) {
                    f($($p),+);
                }
                unsafe {
                    $(let $p = $param::fetch(world_ptr, &mut state.$i);)+
                    call(self, $($p),+);
                }
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

            fn signal_access() -> Option<TypeId> {
                None
            }

            fn after(world: &mut World, state: &mut Self::State) {
                $($param::after(world, &mut state.$i);)+
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
        let mut component_access = Access::default();
        let mut resource_access = Access::default();
        Self::join_component_access(&mut component_access);
        Self::join_resource_access(&mut resource_access);
        FunctionSystem {
            name: self.name(),
            state: None,
            component_access,
            resource_access,
            signal_access: F::signal_access(),
            func: self,
            _a: Default::default(),
        }
    }
}

pub struct Commands<'a> {
    queue: &'a mut Vec<u8>,
}

enum CommandMeta {
    Spawn {
        f: fn(&mut World, *mut u8),
        data_size: usize,
    },
    Despawn(Entity),
    SetComponent {
        f: fn(&mut World, *mut u8, Entity),
        entity: Entity,
        data_size: usize
    },
    RemoveComponent {
        f: fn(&mut World, Entity),
        entity: Entity,
    }
}

impl Commands<'_> {
    #[inline]
    fn copy_data<T>(&mut self, value: &T, index: usize) {
        use std::ptr::NonNull;
        let src = NonNull::from(value).cast::<u8>();
        let dst = unsafe { NonNull::from(&self.queue[0]).cast::<u8>().add(index) };
        unsafe { src.copy_to_nonoverlapping(dst, size_of::<T>()) };
    }

    pub fn spawn<B: EntityBundle>(&mut self, bundle: B) {
        let additional = size_of::<CommandMeta>() + size_of::<B>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);

        let command_meta = CommandMeta::Spawn {
            f: |world, data| {
                let data = data as *mut B;
                let bundle = unsafe { data.read_unaligned() };
                world.spawn(bundle);
            },
            data_size: size_of::<B>(),
        };

        self.copy_data(&command_meta, index);
        self.copy_data(&bundle, index + size_of::<CommandMeta>());

        std::mem::forget(bundle);
    }

    pub fn set_component<C: Component>(&mut self, entity: Entity, component: C) {
        let additional = size_of::<CommandMeta>() + size_of::<C>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);

        let command_meta = CommandMeta::SetComponent {
            f: |world, data, entity| {
                let data = data as *mut C;
                let component = unsafe { data.read_unaligned() };
                world.set_component(entity, component);
            },
            data_size: size_of::<C>(),
            entity
        };

        self.copy_data(&command_meta, index);
        self.copy_data(&component, index + size_of::<CommandMeta>());

        std::mem::forget(component);
    }

    pub fn despawn(&mut self, entity: Entity) {
        let additional = size_of::<CommandMeta>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);

        let command_meta = CommandMeta::Despawn(entity);

        self.copy_data(&command_meta, index);
    }

    pub fn remove_component<C: Component>(&mut self, entity: Entity) {
        let additional = size_of::<CommandMeta>();
        let index = self.queue.len();
        self.queue.resize(self.queue.len() + additional, 0);

        let command_meta = CommandMeta::RemoveComponent {
            f: |world, entity| {
                world.remove_component::<C>(entity);
            },
            entity,
        };

        self.copy_data(&command_meta, index);
    }

    unsafe fn read_command_meta(&self, index: usize) -> CommandMeta {
        use std::ptr::NonNull;
        let ptr = NonNull::from(&self.queue[index]).cast::<CommandMeta>();

        unsafe { ptr.read_unaligned() }
    }

    pub(crate) fn process(&mut self, world: &mut World) {
        let len = self.queue.len();
        let mut cursor = 0;
        while cursor < len {
            let command_meta = unsafe { self.read_command_meta(cursor) };
            cursor += size_of::<CommandMeta>();
            match command_meta {
                CommandMeta::Spawn { f, data_size } => {
                    let ptr = unsafe { (&mut self.queue[0] as *mut u8).add(cursor) };
                    (f)(world, ptr);
                    cursor += data_size;
                },
                CommandMeta::Despawn(entity) => {
                    world.remove(entity);
                },
                CommandMeta::SetComponent { f, entity, data_size } => {
                    let ptr = unsafe { (&mut self.queue[0] as *mut u8).add(cursor) };
                    (f)(world, ptr, entity);
                    cursor += data_size;
                },
                CommandMeta::RemoveComponent { f, entity } => {
                    (f)(world, entity);
                },
            }
        }
        self.queue.clear();
    }
}

impl SystemParam for Commands<'_> {
    type Item<'a> = Commands<'a>;
    type State = Vec<u8>;

    fn init_state(_: &mut World) -> Self::State {
        Vec::new()
    }

    unsafe fn fetch<'a>(_: WorldPtr<'a>, state: &'a mut Self::State) -> Self::Item<'a> {
        Commands {
            queue: state,
        }
    }

    fn after(world: &mut World, state: &mut Self::State) {
        let mut commands = Commands {
            queue: state,
        };
        commands.process(world);
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
