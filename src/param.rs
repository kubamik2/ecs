use std::{any::{TypeId, type_name}, fmt::Display};

use crate::{Commands, Resource, ResourceId, system::SystemHandle, world::WorldPtr};

use super::{access::Access, World};

/// # Safety
/// If a param uses an external resource or component it must declared in join_resource_access and
/// join_component_access accordingly
#[allow(unused_variables)]
pub unsafe trait SystemParam {
    type Item<'a>;
    type State: Send + Sync;
    fn join_component_access(world: &mut World, component_access: &mut Access) -> Result<(), SystemParamError> { Ok(()) }
    fn join_resource_access(world: &mut World, resource_access: &mut Access) -> Result<(), SystemParamError> { Ok(()) }
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
    MissingComponent(&'static str)
}

impl Display for SystemParamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingComponent(name) => f.write_fmt(format_args!("missing component '{}'", name)),
            Self::MissingResource(name) => f.write_fmt(format_args!("missing resource '{}'", name)),
        }
    }
}

#[inline]
pub(crate) fn get_resource_id<R: Resource>(world: &mut World) -> Result<ResourceId, SystemParamError> {
    world.get_resource_id::<R>().ok_or(SystemParamError::MissingResource(type_name::<R>()))
}
