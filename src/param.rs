use std::{any::{TypeId, type_name}, fmt::Display};

use crate::{Commands, Resource, ResourceId, World, access::{AccessBuilder, Conflict}, system::SystemHandle, world::WorldPtr};

/// # Safety
/// If a param uses an external resource or component it must declared in join_resource_access and
/// join_component_access accordingly
#[allow(unused_variables)]
pub unsafe trait SystemParam {
    type Item<'a>;
    type State: Send + Sync;
    fn join_access(world: &mut World, access: &mut AccessBuilder) -> Result<(), SystemParamError> { Ok(()) }
    fn join_trigger_access(trigger_access: &mut Option<TypeId>) {}
    fn init_state(world: &mut World, system_meta: &SystemHandle) -> Result<Self::State, SystemParamError>;
    /// This function will run in parallel
    /// It is only meant to fetch the reference to the data from the world
    /// # Safety
    /// The caller must not modify the world such that it would cause a data race
    unsafe fn fetch<'a>(world_ptr: WorldPtr<'a>, state: &'a mut Self::State, system_meta: &'a SystemHandle<'a>) -> Self::Item<'a>;
    fn after<'state>(commands: &mut Commands<'state>, state: &'state mut Self::State) {}
}

#[derive(Clone, Copy, Debug)]
pub enum SystemParamError {
    MissingResource(&'static str),
    MissingComponent(&'static str),
    Conflict(Conflict),
}

impl Display for SystemParamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingComponent(name) => f.write_fmt(format_args!("missing component '{}'", name)),
            Self::MissingResource(name) => f.write_fmt(format_args!("missing resource '{}'", name)),
            Self::Conflict(conflict) => std::fmt::Display::fmt(conflict, f),
        }
    }
}

#[inline]
pub(crate) fn get_resource_id<R: Resource>(world: &mut World) -> Result<ResourceId, SystemParamError> {
    world.get_resource_id::<R>().ok_or(SystemParamError::MissingResource(type_name::<R>()))
}
